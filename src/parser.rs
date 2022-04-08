use crate::error::Error;
use crate::ops::Operation;
use std::collections::{HashMap, HashSet};
use std::fmt::Formatter;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

pub struct Parser {
    log_file: PathBuf,
    fd_map: HashMap<i32, OpenedFile>,
    files: HashSet<FileDir>, // keep existing files' size
}

#[derive(Clone, Eq, Hash, PartialEq)]
pub enum FileDir {
    File(String, usize), // file path and size
    Dir(String, usize),  // directory path and size
}

impl FileDir {
    pub fn path(&self) -> &str {
        match &self {
            &FileDir::File(ref path, _) => path,
            &FileDir::Dir(ref path, _) => path,
        }
    }

    pub fn size(&self) -> &usize {
        match &self {
            &FileDir::File(_, ref size) => size,
            &FileDir::Dir(_, ref size) => size,
        }
    }
}

impl std::fmt::Display for FileDir {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            &FileDir::File(path, size) => write!(f, "file path: {}, size: {}", path, size),
            &FileDir::Dir(path, size) => write!(f, "directory path {}, size: {}", path, size),
        }
    }
}

#[derive(Debug)]
struct OpenedFile {
    path: String,
    offset: i32,
    size: usize,
}

impl OpenedFile {
    pub fn new(path: String, offset: i32, size: usize) -> Self {
        OpenedFile { path, offset, size }
    }
}

impl Parser {
    pub fn new(log_file: PathBuf) -> Self {
        Parser {
            log_file,
            fd_map: HashMap::new(),
            files: HashSet::new(),
        }
    }

    pub fn parse(&mut self) -> Result<Vec<Operation>, Box<dyn std::error::Error>> {
        let mut operations = vec![];

        let file = File::open(self.log_file.clone())?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line?;

            // filter out the operations
            if line.contains("= -1") || // ops with error result
                line.starts_with("close") || // close op
                line.starts_with("readlink")
            // readlink op
            {
                continue;
            }

            if line.starts_with("openat(") {
                let operation = self.openat(line.as_ref())?;
                operations.push(operation);
            }

            if line.starts_with("fcntl(") {
                let operation = self.fcntl(line.as_ref())?;
                operations.push(operation);
            }

            if line.starts_with("read(") {
                // read op updates the file offset
                operations.push(self.read(line.as_ref())?);
            }

            if line.starts_with("stat(") {
                operations.push(self.stat(line.as_ref())?);
            }

            if line.starts_with("fstat(") {
                operations.push(self.fstat(line.as_ref())?);
            }

            if line.starts_with("statx(") {
                operations.push(self.statx(line.as_ref())?);
            }

            if line.starts_with("statfs(") {
                operations.push(self.statfs(line.as_ref())?);
            }

            if line.starts_with("fstatat64(")
                || line.starts_with("newfstatat(")
                || line.starts_with("fstatat(")
            {
                operations.push(self.fstatat(line.as_ref())?);
            }

            if line.starts_with("pread(") {
                // pread op does not update the file offset
                operations.push(self.pread(line.as_ref())?);
            }

            if line.starts_with("getrandom(") {
                operations.push(self.get_random(line.as_ref())?);
            }

            if line.starts_with("write(") {
                operations.push(self.write(line.as_ref())?);
            }

            if line.starts_with("mkdir(") {
                operations.push(self.mkdir(line.as_ref())?);
            }

            if line.starts_with("unlinkat(") || line.starts_with("unlink(") {
                operations.push(self.unlink(line.as_ref())?);
            }

            if line.starts_with("rename(") {
                operations.push(self.rename(line.as_ref())?);
            }

            if line.starts_with("renameat(") || line.starts_with("renameat2(") {
                operations.push(self.renameat(line.as_ref())?);
            }
        }

        // remove no-ops
        operations.retain(|op| op != &Operation::NoOp);

        Ok(operations)
    }

    // parse an openat line
    fn openat(&mut self, line: &str) -> Result<Operation, Box<dyn std::error::Error>> {
        //
        // an openat line of the strace log is like:
        //   1.   openat(dirfd, "a-path", flags, mode) = 3
        // or
        //   2.   openat(dirfd, "a-path", flags) = 3
        //
        // If the path is absolute, then dirfd is ignored.
        // If dirfd = 'AT_FDCWD', the path is interpreted relative to the current working directory
        // of the calling process.
        // If dirfd is a file descriptor, then the path is relative to the path of the directory
        // described by the file descriptor.
        //

        // the returned file descriptor is after '='
        let fd = line
            .split_at(
                line.rfind("=")
                    .ok_or(Error::NotFound("= from openat line".to_string()))?
                    + 1,
            )
            .1;
        let fd = fd.trim().parse::<i32>()?;

        // extract the input arguments
        let args = self.args(line, "openat")?;

        // extract the path from input arguments
        let mut path = self.path(&args, "openat")?;

        // extract the dirfd
        let dirfd = args
            .split_at(
                args.find(",")
                    .ok_or(Error::NotFound(", from openat line".to_string()))?,
            )
            .0;
        if !dirfd.contains("AT_FDCWD") {
            // dirfd should be a valid file descriptor, so the input path is a relative path.
            path = self.relative_to_absolute(dirfd, &path)?;
        }

        let flags_mode = args
            .split_at(
                args.rfind("\"")
                    .ok_or(Error::NotFound("\" from openat line".to_string()))?
                    + 2,
            )
            .1;

        let flags = if flags_mode.contains(",") {
            // there is a mode in the arguments
            let (flags, _mode) = flags_mode.split_at(
                flags_mode
                    .find(",")
                    .ok_or(Error::NotFound(", from openat line".to_string()))?,
            );
            flags
        } else {
            flags_mode
        };

        let (offset, size) = match self.fd_map.get(&fd) {
            Some(of) => {
                // we have already created an OpenAt operation
                if flags.contains("O_TRUNC") {
                    (0, 0) // the file is opened in truncate mode, so the offset should be 0
                } else {
                    (of.offset, of.size) // the file is opened in non-truncate mode (e.g, append mode), so the offset is the same
                }
            }
            None => {
                // the file is opened for the first time
                (0, 0)
            }
        };

        self.fd_map
            .insert(fd, OpenedFile::new(path.clone(), offset, size));

        let operation = if flags.contains("O_CREAT") {
            Operation::mknod(size, offset, path)
        } else {
            Operation::open_at(offset, path)
        };

        Ok(operation)
    }

    // parse a fcntl line
    fn fcntl(&mut self, line: &str) -> Result<Operation, Box<dyn std::error::Error>> {
        // a fcntl line of the strace log is like:
        //      fcntl(fd, F_DUPFD, FD_CLOEXEC) = 4
        //  or
        //      fcntl(fd, F_DUPFD, FD_CLOEXEC, args) = 4
        //
        //  If the flag is 'F_DUPFD' or 'F_DUPFD_CLOEXEC', the file descriptor fd is duplicated
        //  using the lowest-numbered available file descriptor greater than or equal to arg.
        //  This means a file that was previously referred by fd, now is referred by the
        //  return value of fcntl.

        // the returned file descriptor is after '='
        let dup_fd = line
            .split_at(
                line.rfind("=")
                    .ok_or(Error::NotFound("= from fcntl line".to_string()))?
                    + 1,
            )
            .1;

        // extract the input arguments
        let args = self.args(line, "fcntl")?;

        let parts: Vec<&str> = args.split(",").collect();
        let fd = parts[0].trim().parse::<i32>()?;

        if parts
            .iter()
            .find(|&&val| val.contains("F_DUPFD") || val.contains("F_DUPFD_CLOEXEC"))
            .is_some()
        {
            let fd_of = self
                .fd_map
                .get(&fd)
                .ok_or(Error::NotFound(format!("file descriptor {}", fd)))?;
            let fd_path = fd_of.path.clone();
            let offset = fd_of.offset;
            let size = fd_of.size;

            // add the duplicated fd to the map
            let dup_fd = dup_fd.trim().parse::<i32>()?;

            self.fd_map
                .insert(dup_fd, OpenedFile::new(fd_path, offset, size));
        }

        Ok(Operation::no_op())
    }

    // parse a read line
    fn read(&mut self, line: &str) -> Result<Operation, Box<dyn std::error::Error>> {
        // a read line of the strace log is like:
        //      read(fd, "a-buf", len) = read_len
        //
        // this operation reads len bytes from opened file offset and changes the opened file offset after read

        // extract the input arguments
        let args = self.args(line, "read")?;

        let parts: Vec<&str> = args.split(",").collect();
        let fd = parts[0].trim().parse::<i32>()?;
        let _buf = parts[1].trim().to_string();
        let len = parts[parts.len() - 1].trim().parse::<usize>()?;

        // find the read path based on the file descriptor
        match self.fd_map.get(&fd) {
            Some(opend_file) => {
                let path = opend_file.path.clone();
                let offset = opend_file.offset;
                let size = opend_file.size;

                // update the offset of the opened file in the fd_map
                self.fd_map
                    .insert(fd, OpenedFile::new(path.clone(), offset + len as i32, size));

                Ok(Operation::read(len, offset, path))
            }
            None => {
                // For some reason the fd is not available. One case is having an operation
                // like ioctl(fd, ...) followed by a read(4, ...) operation.
                // We are not tracking hardware-specific calls.
                Ok(Operation::no_op())
            }
        }
    }

    // parse a stat line
    fn stat(&mut self, line: &str) -> Result<Operation, Box<dyn std::error::Error>> {
        // a stat line of the strace log is like:
        //      stat("a-path", {st_mode=S_IFREG|0664, st_size=62, ...}) = 0
        //
        // the file information such as st_size is available between {}

        // extract the input arguments
        let args = self.args(line, "stat")?;

        // extract the path from input arguments
        let path = self.path(&args, "stat")?;

        let file_dir = self.file_dir(&args, &path, "stat")?;
        self.files.insert(file_dir);

        Ok(Operation::stat(path))
    }

    // parse a fstat line
    fn fstat(&mut self, line: &str) -> Result<Operation, Box<dyn std::error::Error>> {
        // a fstat line of the strace log is like:
        //      fstat(fd, {st_mode=S_IFREG|0644, st_size=95921, ...}) = 0
        //
        // the file information such as st_size is available between {}

        // extract the input arguments
        let args = self.args(line, "fstat")?;

        let fd = args
            .split_at(args.find(",").ok_or(Error::NotFound("(".to_string()))?)
            .0;
        let fd = fd.trim().parse::<i32>()?;

        // find the path based on the file descriptor
        match self.fd_map.get(&fd) {
            Some(opend_file) => {
                let path = opend_file.path.clone();

                let file_dir = self.file_dir(&args, &path, "fstat")?;
                self.files.insert(file_dir);

                Ok(Operation::fstat(path))
            }
            None => Ok(Operation::no_op()),
        }
    }

    // parse a statx line
    fn statx(&mut self, line: &str) -> Result<Operation, Box<dyn std::error::Error>> {
        // a statx line of the strace log is like one of the followings:
        //      statx(dirfd, "", AT_STATX_SYNC_AS_STAT|AT_EMPTY_PATH, STATX_ALL, {stx_mask=STATX_ALL|0x1000, stx_attributes=0, stx_mode=S_IFREG|0644, stx_size=1335, ...}) = 0
        // or
        //      statx(dirfd, "a-path", AT_STATX_SYNC_AS_STAT, STATX_ALL, {stx_mask=STATX_ALL|0x1000, stx_attributes=0, stx_mode=S_IFREG|0644, stx_size=19153, ...}) = 0
        //
        // If the path is absolute, then dirfd is ignored.
        // If dirfd = 'AT_FDCWD', the path is interpreted relative to the current working directory
        // of the calling process.
        // If dirfd is a file descriptor, then the path is relative to the path of the directory
        // described by the file descriptor.
        //
        // the file information such as stx_size is available between {}

        // extract the input arguments
        let args = self.args(line, "statx")?;

        // extract the path from input arguments
        let mut path = self.path(&args, "statx")?;

        // extract the dirfd
        let dirfd = args
            .split_at(
                args.find(",")
                    .ok_or(Error::NotFound(", from openat line".to_string()))?,
            )
            .0;
        if !dirfd.contains("AT_FDCWD") {
            // dirfd should be a valid file descriptor, so the input path is a relative path.
            path = self.relative_to_absolute(dirfd, &path)?;
        }

        let file_dir = self.file_dir(&args, &path, "statx")?;
        self.files.insert(file_dir);

        Ok(Operation::statx(path))
    }

    // parse a fstatat line
    fn fstatat(&mut self, line: &str) -> Result<Operation, Box<dyn std::error::Error>> {
        // a fstatat line of the strace log is like one of the followings:
        //      fstatat(dirfd, "", {st_mode=S_IFDIR|0775, st_size=4096, ...}, flags) = 0
        // or
        //      fstatat(dirfd, "a-path", {st_mode=S_IFDIR|0775, st_size=4096, ...}, flags) = 0
        //
        // If the path is absolute, then dirfd is ignored.
        // If dirfd is 'AT_FDCWD', the path is interpreted relative to the current working directory
        // of the calling process.
        // If dirfd is a file descriptor, then the path is relative to the path of the directory
        // described by the file descriptor.
        //
        // the file information such as st_size is available between {}

        // extract the input arguments
        let args = self.args(line, "fstatat")?;

        // extract the path from input arguments
        let mut path = self.path(&args, "fstatat")?;

        // extract the dirfd
        let dirfd = args
            .split_at(
                args.find(",")
                    .ok_or(Error::NotFound(", from openat line".to_string()))?,
            )
            .0;
        if !dirfd.contains("AT_FDCWD") {
            // dirfd should be a valid file descriptor, so the input path is a relative path.
            path = self.relative_to_absolute(dirfd, &path)?;
        }

        let file_dir = self.file_dir(&args, &path, "fstatat")?;
        self.files.insert(file_dir);

        Ok(Operation::fstatat(path))
    }

    // parse a statfs line
    fn statfs(&mut self, line: &str) -> Result<Operation, Box<dyn std::error::Error>> {
        // a statfs line of the strace log is like:
        //  statfs("a-path", {f_type=EXT2_SUPER_MAGIC, f_bsize=4096,
        //   f_blocks=114116168, f_bfree=60978756, f_bavail=55164536, f_files=29057024,
        //   f_ffree=27734180, f_fsid={val=[991782359, 1280028847]}, f_namelen=255, f_frsize=4096,
        //   f_flags=ST_VALID|ST_RELATIME}) = 0
        //
        // here we just care about the path and don't need the outputs

        // extract the input arguments
        let args = self.args(line, "statfs")?;

        // extract the path from input arguments
        let path = self.path(&args, "statfs")?;

        Ok(Operation::statfs(path))
    }

    // parse a read line
    fn pread(&mut self, line: &str) -> Result<Operation, Box<dyn std::error::Error>> {
        // a pread (or pread64) line of the strace log is like:
        //  pread(fd, "a-buf", len, offset) = len
        //
        // the operation reads len bytes from input offset and does not change the opened file offset after read

        // extract the input arguments
        let args = self.args(line, "pread")?;

        let parts: Vec<&str> = args.split(",").collect();
        let fd = parts[0].trim().parse::<i32>()?;
        let _buf = parts[1].trim().to_string();
        let len = parts[2].trim().parse::<usize>()?;
        let offset = parts[parts.len() - 1].trim().parse::<i32>()?;

        // find the read path based on the file descriptor
        match self.fd_map.get(&fd) {
            Some(opend_file) => Ok(Operation::read(len, offset, opend_file.path.clone())),
            None => {
                // For some reason the fd is not available. One case is having an operation
                // like ioctl(fd, ...) followed by a read(4, ...) operation.
                // We are not tracking hardware-specific calls.
                Ok(Operation::no_op())
            }
        }
    }

    // parse a write line
    fn write(&mut self, line: &str) -> Result<Operation, Box<dyn std::error::Error>> {
        // a write line of the strace log is like:
        //  write(fd, "a-string", len) = write_len

        // extract the input arguments
        let args = self.args(line, "write")?;

        let parts: Vec<&str> = args.split(",").collect();
        let fd = parts[0].trim().parse::<i32>()?;
        let content = parts[1].trim().to_string();
        let len = parts[parts.len() - 1].trim().parse::<usize>()?;

        if fd == 0 || fd == 1 || fd == 2 {
            // write to stdin, or stdout, or stderr
            return Ok(Operation::no_op());
        }

        // find the write path based on the file descriptor
        match self.fd_map.get(&fd) {
            Some(of) => {
                let path = of.path.clone();
                let offset = of.offset;
                let size = of.size;

                let op = Operation::write(content, len, offset, path.clone());

                // update the offset and size of the opened file in the fd_map
                self.fd_map
                    .insert(fd, OpenedFile::new(path, offset + len as i32, size + len));

                Ok(op)
            }
            None => {
                // as the file descriptor does not exist, the write operation is probably writing
                // to the STDOUT, so do not generate an operation
                Ok(Operation::no_op())
            }
        }
    }

    // parse a write line
    fn mkdir(&mut self, line: &str) -> Result<Operation, Box<dyn std::error::Error>> {
        // a mkdir line of the strace log is like:
        //  mkdir("a-path", mode) = 0

        // extract the input arguments
        let args = self.args(line, "mkdir")?;

        // extract the path from input arguments
        let path = self.path(&args, "mkdir")?;

        let mode = args
            .split_at(args.rfind(",").ok_or(Error::NotFound("\"".to_string()))? + 1)
            .1
            .trim();

        Ok(Operation::mkdir(path, mode.to_string()))
    }

    // parse a unlink line
    fn unlink(&mut self, line: &str) -> Result<Operation, Box<dyn std::error::Error>> {
        // a unlink line of the strace log is like one of the followings:
        //      unlinkat(dirfd, "", AT_REMOVEDIR) = 0
        // or
        //      fstatat(dirfd, "a-path", {st_mode=S_IFDIR|0775, st_size=4096, ...}, flags) = 0
        //
        // If the path is absolute, then dirfd is ignored.
        // If dirfd is 'AT_FDCWD', the path is interpreted relative to the current working directory
        // of the calling process.
        // If dirfd is a file descriptor, then the path is relative to the path of the directory
        // described by the file descriptor.
        //

        // extract the input arguments
        let args = self.args(line, "unlink")?;

        // extract the path from input arguments
        let mut path = self.path(&args, "unlink")?;

        // extract the dirfd
        let dirfd = args
            .split_at(
                args.find(",")
                    .ok_or(Error::NotFound(", from openat line".to_string()))?,
            )
            .0;
        if !dirfd.contains("AT_FDCWD") {
            // dirfd should be a valid file descriptor, so the input path is a relative path.
            path = self.relative_to_absolute(dirfd, &path)?;
        }

        Ok(Operation::remove(path))
    }

    // parse a rename line
    fn rename(&mut self, line: &str) -> Result<Operation, Box<dyn std::error::Error>> {
        // a rename line of the strace log is like one of the followings:
        //      rename("old-path", "new-path") = 0
        //

        // extract the input arguments
        let args = self.args(line, "rename")?;

        let (old, new) = args.split_at(
            args.find(",")
                .ok_or(Error::NotFound("= from rename line".to_string()))?,
        );

        let old = self.path(&old, "rename")?;
        let new = self.path(&new, "rename")?;

        Ok(Operation::rename(old, new))
    }

    // parse a renameat line
    fn renameat(&mut self, line: &str) -> Result<Operation, Box<dyn std::error::Error>> {
        // a renameat line of the strace log is like one of the followings:
        //      renameat2(dirfd, "o-path", dirfd, "n-path", RENAME_NOREPLACE) = 0
        //
        // If the path is absolute, then dirfd is ignored.
        // If dirfd is 'AT_FDCWD', the path is interpreted relative to the current working directory
        // of the calling process.
        // If dirfd is a file descriptor, then the path is relative to the path of the directory
        // described by the file descriptor.
        //

        // extract the input arguments
        let args = self.args(line, "rename")?;

        let parts: Vec<&str> = args.split(",").collect();
        let dirfd1 = parts[0];
        let old = parts[1];
        let mut old = self.path(&old, "renameat")?;

        let dirfd2 = parts[2];
        let new = parts[3];
        let mut new = self.path(&new, "renameat")?;

        if !dirfd1.contains("AT_FDCWD") {
            // dirfd should be a valid file descriptor, so the input path is a relative path.
            old = self.relative_to_absolute(dirfd1, &old)?;
        }

        if !dirfd2.contains("AT_FDCWD") {
            // dirfd should be a valid file descriptor, so the input path is a relative path.
            new = self.relative_to_absolute(dirfd2, &new)?;
        }

        Ok(Operation::rename(old, new))
    }

    // parse a getrandom line
    fn get_random(&mut self, line: &str) -> Result<Operation, Box<dyn std::error::Error>> {
        // a getrandom line of the strace log is like:
        //  getrandom("a-buf", len, flags) = random_bytes_len

        // extract the input arguments
        let args = self.args(line, "getrandom")?;

        let parts: Vec<&str> = args.split(",").collect();
        let _buf = parts[0].to_string();
        let len = parts[1].trim().parse::<usize>()?;
        let _flags = parts[parts.len() - 1].trim().to_string();

        Ok(Operation::get_random(len))
    }

    // extract the input args in-between ( and ) from the input string
    fn args(&self, str: &str, callee: &str) -> Result<String, Box<dyn std::error::Error>> {
        // the body is before '='
        let body = str
            .split_at(
                str.rfind("=")
                    .ok_or(Error::NotFound(format!("= from {} line", callee)))?,
            )
            .0;

        let args = body
            .split_at(
                body.find("(")
                    .ok_or(Error::NotFound(format!("( from {} line", callee)))?
                    + 1,
            )
            .1;
        Ok(args
            .split_at(
                args.rfind(")")
                    .ok_or(Error::NotFound(format!(") from {} line", callee)))?,
            )
            .0
            .to_string())
    }

    // extract a path in-between " and " from the input string
    fn path(&self, str: &str, callee: &str) -> Result<String, Box<dyn std::error::Error>> {
        let path = str
            .split_at(
                str.find("\"")
                    .ok_or(Error::NotFound(format!("\" from {} line", callee)))?
                    + 1,
            )
            .1;
        Ok(path
            .split_at(
                path.rfind("\"")
                    .ok_or(Error::NotFound(format!("\" from {} line", callee)))?,
            )
            .0
            .to_string())
    }

    fn relative_to_absolute(
        &self,
        dirfd: &str,
        relative: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        // dirfd should be a valid file descriptor
        let dirfd = dirfd.trim().parse::<i32>()?;
        let dirfd_path = self
            .fd_map
            .get(&dirfd)
            .ok_or(Error::NotFound(format!("file descriptor {}", dirfd)))?
            .path
            .clone();
        // create the absolute path
        Ok(format!("{}{}", dirfd_path, relative))
    }

    fn file_dir(
        &self,
        str: &str,
        path: &str,
        callee: &str,
    ) -> Result<FileDir, Box<dyn std::error::Error>> {
        // extract the file size
        let st_size: String = str
            .trim()
            .split(",")
            .filter(|stat| stat.contains("st_size") || stat.contains("stx_size"))
            .map(|str| str.to_string())
            .collect();
        let file_size = st_size
            .split_at(
                st_size
                    .find("=")
                    .ok_or(Error::NotFound(format!("= from {} line", callee)))?
                    + 1,
            )
            .1
            .trim()
            .parse::<usize>()?;

        // extract the file type
        let st_mode: String = str
            .trim()
            .split(",")
            .filter(|stat| stat.contains("st_mode") || stat.contains("stx_mode"))
            .map(|str| str.to_string())
            .collect();
        let file_type = st_mode
            .split_at(
                st_mode
                    .find("=")
                    .ok_or(Error::NotFound(format!("= from {} line", callee)))?
                    + 1,
            )
            .1
            .trim();

        if file_type.contains("S_IFREG") {
            Ok(FileDir::File(path.to_string(), file_size))
        } else {
            Ok(FileDir::Dir(path.to_string(), file_size))
        }
    }

    pub fn accessed_files(&self) -> Result<HashSet<FileDir>, Box<dyn std::error::Error>> {
        Ok(self.files.clone())
    }
}

#[cfg(test)]
mod test {
    use crate::parser::Parser;
    use crate::Operation;
    use std::path::PathBuf;

    #[test]
    fn openat() -> Result<(), Box<dyn std::error::Error>> {
        let mut parser = Parser::new(PathBuf::new());
        let line = "openat(AT_FDCWD, \"a_path\", O_RDONLY|O_CLOEXEC) = 9".to_string();
        let operation = parser.openat(line.as_ref())?;
        assert_eq!(operation, Operation::OpenAt("a_path".to_string(), 0));

        let line =
            "openat(AT_FDCWD, \"another_path\", O_RDONLY|O_CREAT|O_CLOEXEC, 0666) = 7".to_string();
        let operation = parser.openat(line.as_ref())?;
        assert_eq!(
            operation,
            Operation::Mknod("another_path".to_string(), 0, 0)
        );

        Ok(())
    }

    #[test]
    fn read() -> Result<(), Box<dyn std::error::Error>> {
        let mut parser = Parser::new(PathBuf::new());
        let openat_line = "openat(AT_FDCWD, \"a_path\", O_RDONLY|O_CLOEXEC) = 3".to_string();
        let _operation = parser.openat(openat_line.as_ref())?;

        let read_line1 = "read(3, buf, 50) = 50".to_string();
        let read_op1 = parser.read(read_line1.as_ref())?;
        assert_eq!(read_op1, Operation::Read("a_path".to_string(), 0, 50));

        let read_line2 = "read(3, buf, 20) = 20".to_string();
        let read_op2 = parser.read(read_line2.as_ref())?;
        assert_eq!(read_op2, Operation::Read("a_path".to_string(), 50, 20));

        Ok(())
    }

    #[test]
    fn pread() -> Result<(), Box<dyn std::error::Error>> {
        let mut parser = Parser::new(PathBuf::new());
        let openat_line = "openat(AT_FDCWD, \"a_path\", O_RDONLY|O_CLOEXEC) = 3".to_string();
        let _operation = parser.openat(openat_line.as_ref())?;

        let read_line1 = "pread(3, buf, 50, 100) = 50".to_string();
        let pread_op1 = parser.pread(read_line1.as_ref())?;
        assert_eq!(pread_op1, Operation::Read("a_path".to_string(), 100, 50));

        let read_line2 = "pread(3, buf, 20, 500) = 20".to_string();
        let pread_op2 = parser.pread(read_line2.as_ref())?;
        assert_eq!(pread_op2, Operation::Read("a_path".to_string(), 500, 20));

        // the previous pread ops should not update the opened file offset
        let read_line3 = "read(3, buf, 20) = 20".to_string();
        let read_op = parser.read(read_line3.as_ref())?;
        assert_eq!(read_op, Operation::Read("a_path".to_string(), 0, 20));

        Ok(())
    }

    #[test]
    fn write() -> Result<(), Box<dyn std::error::Error>> {
        let mut parser = Parser::new(PathBuf::new());
        let line = "openat(AT_FDCWD, \"a_path\", O_RDONLY|O_CREAT) = 5".to_string();
        let operation = parser.openat(line.as_ref())?;
        assert_eq!(operation, Operation::Mknod("a_path".to_string(), 0, 0));

        // first write
        let write_line1 = "write(5, some content here, 17) = 17".to_string();
        let write_op1 = parser.write(write_line1.as_ref())?;
        assert_eq!(
            write_op1,
            Operation::Write("a_path".to_string(), 0, 17, "some content here".to_string())
        );

        // second write
        let write_line2 = "write(5, hello, 5) = 5".to_string();
        let write_op2 = parser.write(write_line2.as_ref())?;
        assert_eq!(
            write_op2,
            Operation::Write("a_path".to_string(), 17, 5, "hello".to_string())
        );

        // open the file one more time to check the offset
        let line = "openat(AT_FDCWD, \"a_path\", O_RDONLY|O_CREAT) = 5".to_string();
        let operation = parser.openat(line.as_ref())?;
        assert_eq!(
            operation,
            Operation::Mknod("a_path".to_string(), 17 + 5, 17 + 5)
        );

        // now open the file with truncate flag, which should zero the size and offset
        let line = "openat(AT_FDCWD, \"a_path\", O_RDONLY|O_CREAT|O_TRUNC) = 5".to_string();
        let operation = parser.openat(line.as_ref())?;
        assert_eq!(operation, Operation::Mknod("a_path".to_string(), 0, 0));

        // write after truncate
        let write_line2 = "write(5, some other content here, 10) = 10".to_string();
        let write_op2 = parser.write(write_line2.as_ref())?;
        assert_eq!(
            write_op2,
            Operation::Write(
                "a_path".to_string(),
                0,
                10,
                "some other content here".to_string()
            )
        );

        Ok(())
    }

    #[test]
    fn get_random() -> Result<(), Box<dyn std::error::Error>> {
        let mut parser = Parser::new(PathBuf::new());
        let line = "getrandom(a_buf, 16, GRND_NONBLOCK) = 16".to_string();
        let operation = parser.get_random(line.as_ref())?;
        assert_eq!(operation, Operation::GetRandom(16));

        Ok(())
    }

    #[test]
    fn fstat() -> Result<(), Box<dyn std::error::Error>> {
        let mut parser = Parser::new(PathBuf::new());
        let openat_line = "openat(AT_FDCWD, \"a_path\", O_RDONLY|O_CLOEXEC) = 3".to_string();
        let _operation = parser.openat(openat_line.as_ref())?;

        let fstat_line = "fstat(3, {st_mode=S_IFREG|0644, st_size=95921, ...}) = 0".to_string();
        let fstat_op = parser.fstat(fstat_line.as_ref())?;
        assert_eq!(fstat_op, Operation::Fstat("a_path".to_string()));

        assert_eq!(parser.files.len(), 1);
        let file_size = parser
            .files
            .iter()
            .next()
            .expect("failed to get the first element from hashset")
            .size();
        assert_eq!(*file_size, 95921 as usize);

        Ok(())
    }

    #[test]
    fn mkdir() -> Result<(), Box<dyn std::error::Error>> {
        let mut parser = Parser::new(PathBuf::new());

        let mkdir_line = "mkdir(\"a_path\", 0777) = 0".to_string();
        let mkdir_op = parser.mkdir(mkdir_line.as_ref())?;
        assert_eq!(
            mkdir_op,
            Operation::Mkdir("a_path".to_string(), "0777".to_string())
        );

        Ok(())
    }

    #[test]
    fn rename() -> Result<(), Box<dyn std::error::Error>> {
        let mut parser = Parser::new(PathBuf::new());
        let line = "rename(\"old_path\", \"new_path\") = 0".to_string();
        let operation = parser.rename(line.as_ref())?;
        assert_eq!(
            operation,
            Operation::Rename("old_path".to_string(), "new_path".to_string())
        );

        Ok(())
    }

    #[test]
    fn renameat() -> Result<(), Box<dyn std::error::Error>> {
        let mut parser = Parser::new(PathBuf::new());
        let line =
            "renameat2(AT_FDCWD, \"old_path\", AT_FDCWD, \"new_path\", RENAME_NOREPLACE) = 0"
                .to_string();
        let operation = parser.renameat(line.as_ref())?;
        assert_eq!(
            operation,
            Operation::Rename("old_path".to_string(), "new_path".to_string())
        );

        Ok(())
    }
}
