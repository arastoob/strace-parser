use crate::deps::DependencyGraph;
use crate::error::Error;
use crate::file::File;
use crate::op::Operation;
use crate::process::Process;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fmt::Formatter;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::Arc;

pub struct Parser {
    log_file: PathBuf,
    fd_map: HashMap<i32, OpenedFile>, // a map from file descriptor to a OpenedFile struct
    existing_files: HashSet<FileDir>, // keep existing files info
    accessed_files: HashMap<String, Arc<File>>, // all the files and directories accessed by processes
    ongoing_ops: HashMap<String, String>, // keeping the unfinished operations for each process
}

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
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

#[derive(Debug, PartialEq)]
enum Parts {
    Unfinished(usize, String),
    Finished(usize, String, String, String),
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
            existing_files: HashSet::new(),
            accessed_files: HashMap::new(),
            ongoing_ops: HashMap::new(),
        }
    }

    pub fn parse(&mut self) -> Result<Vec<Process>, Box<dyn std::error::Error>> {
        let mut processes: Vec<Process> = vec![];

        let file = std::fs::File::open(self.log_file.clone())?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line?;

            // make sure the strace logs has process ids for each logged operation
            if !self.has_pid(&line)? {
                return Err(Box::new(Error::ParseError(line.to_string())));
            }

            // filter out the operations
            if line.contains("= -1") || // ops with error result
                line.starts_with("close") || // close op
                line.starts_with("readlink") || // readlink op
                line.contains("---") ||
                line.contains("+++")
            {
                continue;
            }

            match self.parts(&line)? {
                Parts::Unfinished(_, _) => continue,
                Parts::Finished(pid, op, args, ret) => {
                    if processes.iter().find(|p| p.pid() == pid).is_none() {
                        // generate the process for the first time and add it to the list of processes
                        processes.push(Process::new(pid));
                    }

                    let process = processes
                        .iter_mut()
                        .find(|p| p.pid() == pid)
                        .ok_or(Error::NotFound(format!("pid {}", pid)))?;

                    match op.as_ref() {
                        "openat" => {
                            for operation in self.openat(args, ret)? {
                                process.add_op(operation.into());
                            }
                        }
                        "fcntl" => {
                            process.add_op(self.fcntl(args, ret)?.into());
                        }
                        "read" => {
                            // read op updates the file offset
                            process.add_op(self.read(args)?.into());
                        }
                        "stat" => {
                            process.add_op(self.stat(args)?.into());
                        }
                        "fstat" => {
                            process.add_op(self.fstat(args)?.into());
                        }
                        "statx" => {
                            process.add_op(self.statx(args)?.into());
                        }
                        "statfs" => {
                            process.add_op(self.statfs(args)?.into());
                        }
                        op if op == "fstatat64" || op == "newfstatat" || op == "fstatat" => {
                            process.add_op(self.fstatat(args)?.into());
                        }
                        "pread" => {
                            process.add_op(self.pread(args)?.into());
                        }
                        "getrandom" => {
                            process.add_op(self.get_random(args)?.into());
                        }
                        "write" => {
                            process.add_op(self.write(args)?.into());
                        }
                        "mkdir" => {
                            process.add_op(self.mkdir(args)?.into());
                        }
                        "unlinkat" => {
                            process.add_op(self.unlink(args)?.into());
                        }
                        "rename" => {
                            process.add_op(self.rename(args)?.into());
                        }
                        op if op == "renameat" || op == "renameat2" => {
                            process.add_op(self.renameat(args)?.into());
                        }
                        "clone" => {
                            process.add_op(self.clone(ret)?.into());
                        }
                        _ => {}
                    }
                }
            }
        }

        let dep_dag = DependencyGraph::new(processes)?;
        // mark the dependencies
        dep_dag.mark_dependencies()?;
        // get the process list with the pre lists per each operation
        let processes = dep_dag.processes();

        Ok(processes)
    }

    // parse an openat line
    fn openat(
        &mut self,
        args: String,
        ret: String,
    ) -> Result<Vec<Operation>, Box<dyn std::error::Error>> {
        //
        // int openat(int dirfd, const char *path, int flags, mode_t mode);
        //
        // If the path is absolute, then dirfd is ignored.
        // If dirfd = 'AT_FDCWD', the path is interpreted relative to the current working directory
        // of the calling process.
        // If dirfd is a file descriptor, then the path is relative to the path of the directory
        // described by the file descriptor.
        //
        // The important flags are:
        //      O_APPEND: The file is opened in append mode.  Before each write(2), the file offset
        //                is positioned at the end of the file, as if with lseek(2).
        //      O_CREAT: If pathname does not exist, create it as a regular file.
        //      O_TRUNC: If the file already exists and is a regular file and the access mode allows
        //               writing (i.e., is O_RDWR or O_WRONLY) it will be truncated to length 0.

        let fd = ret.trim().parse::<i32>()?;

        // extract the input arguments
        // let args = self.args(line, "openat")?;

        // extract the path from input arguments
        let mut path = self.path(&args, "openat")?;

        if !PathBuf::from(path.clone()).is_absolute() {
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

        let mut operations = vec![];

        if flags.contains("O_CREAT") {
            operations.push(Operation::mknod(self.file(&path).clone()));
        }

        if flags.contains("O_TRUNC") {
            operations.push(Operation::truncate(self.file(&path).clone()));

            self.fd_map.insert(fd, OpenedFile::new(path.clone(), 0, 0));
        }

        let offset = match self.fd_map.get(&fd) {
            Some(of) => {
                // we have already seen the file
                let offset = of.offset;
                let size = of.size;
                if flags.contains("O_APPEND") {
                    // the file offset should point to the end
                    self.fd_map
                        .insert(fd, OpenedFile::new(path.clone(), size as i32, size));
                    size as i32
                } else {
                    self.fd_map
                        .insert(fd, OpenedFile::new(path.clone(), offset, size));
                    offset
                }
            }
            None => {
                // the file is opened for the first time
                self.fd_map.insert(fd, OpenedFile::new(path.clone(), 0, 0));
                0
            }
        };

        // finally, create the openat operation
        operations.push(Operation::open_at(self.file(&path).clone(), offset));

        Ok(operations)
    }

    // parse a fcntl line
    fn fcntl(
        &mut self,
        args: String,
        ret: String,
    ) -> Result<Operation, Box<dyn std::error::Error>> {
        // int fcntl(int fd, int cmd, ... /* arg */ );
        // performs one of the operations on the open file descriptor fd.  The operation is determined by cmd.
        //
        // Example:
        //      fcntl(fd, F_DUPFD, FD_CLOEXEC) = 4
        //  or
        //      fcntl(fd, F_DUPFD, FD_CLOEXEC, args) = 4
        //
        //  If the flag is 'F_DUPFD' or 'F_DUPFD_CLOEXEC', the file descriptor fd is duplicated
        //  using the lowest-numbered available file descriptor greater than or equal to arg.
        //  This means a file that was previously referred by fd, now is referred by the
        //  return value of fcntl.

        // the returned file descriptor is after '='
        let dup_fd = ret.trim().parse::<i32>()?;

        let parts: Vec<&str> = args.split(",").collect();
        let fd = parts[0].trim().parse::<i32>()?;

        if parts
            .iter()
            .find(|&&val| val.contains("F_DUPFD") || val.contains("F_DUPFD_CLOEXEC"))
            .is_some()
        {
            if let Some(fd_of) = self
                .fd_map
                .get(&fd) {
                let fd_path = fd_of.path.clone();
                let offset = fd_of.offset;
                let size = fd_of.size;

                // add the duplicated fd to the map
                self.fd_map
                    .insert(dup_fd, OpenedFile::new(fd_path, offset, size));
            }
        }

        Ok(Operation::no_op())
    }

    // parse a read line
    fn read(&mut self, args: String) -> Result<Operation, Box<dyn std::error::Error>> {
        // ssize_t read(int fd, void *buf, size_t count);
        // attempts to read up to count bytes from file descriptor fd into the buffer starting at buf.
        //
        // Example:
        //      read(fd, "a-buf", len) = read_len
        //

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

                Ok(Operation::read(self.file(&path).clone(), len, offset))
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
    fn stat(&mut self, args: String) -> Result<Operation, Box<dyn std::error::Error>> {
        // int stat(const char *pathname, struct stat *statbuf);
        // display file or file system status.
        //
        // Example:
        //      stat("a-path", {st_mode=S_IFREG|0664, st_size=62, ...}) = 0
        //
        // the file information such as st_size is available between {}

        // extract the path from input arguments
        let path = self.path(&args, "stat")?;

        match self.file_dir(&args, &path, &format!("stat: {}", args)) {
            Ok(file_dir) => {
                self.existing_files.insert(file_dir);
                Ok(Operation::stat(self.file(&path).clone()))
            },
            Err(err) => {
                if err.to_string().contains("invalid type") {
                    Ok(Operation::no_op())
                } else {
                    Err(err)
                }
            }
        }

    }

    // parse a fstat line
    fn fstat(&mut self, args: String) -> Result<Operation, Box<dyn std::error::Error>> {
        // int fstat(int fd, struct stat *statbuf);
        // return information about a file, in the buffer pointed to by statbuf
        //
        // Example:
        //      fstat(fd, {st_mode=S_IFREG|0644, st_size=95921, ...}) = 0
        //
        // the file information such as st_size is available between {}

        let fd = args
            .split_at(args.find(",").ok_or(Error::NotFound("(".to_string()))?)
            .0;
        let fd = fd.trim().parse::<i32>()?;

        // find the path based on the file descriptor
        match self.fd_map.get(&fd) {
            Some(opend_file) => {
                let path = opend_file.path.clone();

                match self.file_dir(&args, &path, &format!("fstat: {}", args)) {
                    Ok(file_dir) => {
                        self.existing_files.insert(file_dir);
                        Ok(Operation::fstat(self.file(&path).clone()))
                    },
                    Err(err) => {
                        if err.to_string().contains("invalid type") {
                            Ok(Operation::no_op())
                        } else {
                            Err(err)
                        }
                    }
                }
            }
            None => Ok(Operation::no_op()),
        }
    }

    // parse a statx line
    fn statx(&mut self, args: String) -> Result<Operation, Box<dyn std::error::Error>> {
        // int statx(int dirfd, const char *pathname, int flags,
        //                  unsigned int mask, struct statx *statxbuf);
        //  returns information about a file, storing it in the buffer pointed to by statxbuf.
        //
        // Example:
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

        // extract the path from input arguments
        let mut path = self.path(&args, "statx")?;

        if !PathBuf::from(path.clone()).is_absolute() {
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
        }

        match self.file_dir(&args, &path, &format!("statx: {}", args)) {
            Ok(file_dir) => {
                self.existing_files.insert(file_dir);
                Ok(Operation::statx(self.file(&path).clone()))
            },
            Err(err) => {
                if err.to_string().contains("invalid type") {
                    Ok(Operation::no_op())
                } else {
                    Err(err)
                }
            }
        }
    }

    // parse a fstatat line
    fn fstatat(&mut self, args: String) -> Result<Operation, Box<dyn std::error::Error>> {
        // int fstatat(int dirfd, const char *pathname, struct stat *statbuf,
        //                    int flags);
        //  return information about a file, in the buffer pointed to by statbuf
        //
        // Example:
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

        // extract the path from input arguments
        let mut path = self.path(&args, "fstatat")?;

        if !PathBuf::from(path.clone()).is_absolute() {
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
        }

        let file_dir = self.file_dir(&args, &path, &format!("fstatat: {}", args))?;
        self.existing_files.insert(file_dir);

        match self.file_dir(&args, &path, &format!("fstatat: {}", args)) {
            Ok(file_dir) => {
                self.existing_files.insert(file_dir);
                Ok(Operation::fstatat(self.file(&path).clone()))
            },
            Err(err) => {
                if err.to_string().contains("invalid type") {
                    Ok(Operation::no_op())
                } else {
                    Err(err)
                }
            }
        }
    }

    // parse a statfs line
    fn statfs(&mut self, args: String) -> Result<Operation, Box<dyn std::error::Error>> {
        // int statfs(const char *path, struct statfs *buf);
        // returns information about a mounted filesystem
        //
        // Example:
        //  statfs("a-path", {f_type=EXT2_SUPER_MAGIC, f_bsize=4096,
        //   f_blocks=114116168, f_bfree=60978756, f_bavail=55164536, f_files=29057024,
        //   f_ffree=27734180, f_fsid={val=[991782359, 1280028847]}, f_namelen=255, f_frsize=4096,
        //   f_flags=ST_VALID|ST_RELATIME}) = 0
        //
        // here we just care about the path and don't need the outputs

        // extract the path from input arguments
        let path = self.path(&args, "statfs")?;

        Ok(Operation::statfs(self.file(&path).clone()))
    }

    // parse a read line
    fn pread(&mut self, args: String) -> Result<Operation, Box<dyn std::error::Error>> {
        // ssize_t pread(int fd, void *buf, size_t count, off_t offset);
        // reads  up  to  count  bytes from file descriptor fd at offset offset
        // (from the start of the file) into the buffer starting at buf.  The file offset is not changed.
        //
        // Example:
        //  pread(fd, "a-buf", len, offset) = len
        //
        // the operation reads len bytes from input offset and does not change the opened file offset after read

        let parts: Vec<&str> = args.split(",").collect();
        let fd = parts[0].trim().parse::<i32>()?;
        let _buf = parts[1].trim().to_string();
        let len = parts[2].trim().parse::<usize>()?;
        let offset = parts[parts.len() - 1].trim().parse::<i32>()?;

        // find the read path based on the file descriptor
        let path = match self.fd_map.get(&fd) {
            Some(opend_file) => opend_file.path.clone(),
            None => {
                // For some reason the fd is not available. One case is having an operation
                // like ioctl(fd, ...) followed by a read(4, ...) operation.
                // We are not tracking hardware-specific calls.
                return Ok(Operation::no_op());
            }
        };

        Ok(Operation::read(self.file(&path).clone(), len, offset))
    }

    // parse a write line
    fn write(&mut self, args: String) -> Result<Operation, Box<dyn std::error::Error>> {
        // ssize_t write(int fd, const void *buf, size_t count);
        // writes up to count bytes from the buffer starting at buf to the file referred to
        // by the file descriptor fd.
        //
        // Example:
        //  write(fd, "a-string", len) = write_len

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

                let op = Operation::write(self.file(&path).clone(), content, len, offset);

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
    fn mkdir(&mut self, args: String) -> Result<Operation, Box<dyn std::error::Error>> {
        // int mkdir(const char *pathname, mode_t mode);
        // attempts to create a directory named pathname.
        //
        // Example:
        //  mkdir("a-path", mode) = 0

        // extract the path from input arguments
        let path = self.path(&args, "mkdir")?;

        let mode = args
            .split_at(args.rfind(",").ok_or(Error::NotFound("\"".to_string()))? + 1)
            .1
            .trim();

        Ok(Operation::mkdir(self.file(&path).clone(), mode.to_string()))
    }

    // parse a unlink line
    fn unlink(&mut self, args: String) -> Result<Operation, Box<dyn std::error::Error>> {
        // int unlinkat(int dirfd, const char *pathname, int flags);
        // deletes a name from the filesystem.  If that name was the last link to a file and no
        // processes have the file open, the file is deleted and the space it was using is made
        // available for reuse.
        //
        // Example:
        //      unlinkat(dirfd, "", AT_REMOVEDIR) = 0
        // or
        //      unlinkat(dirfd, "a-path", {st_mode=S_IFDIR|0775, st_size=4096, ...}, flags) = 0
        //
        // If the path is absolute, then dirfd is ignored.
        // If dirfd is 'AT_FDCWD', the path is interpreted relative to the current working directory
        // of the calling process.
        // If dirfd is a file descriptor, then the path is relative to the path of the directory
        // described by the file descriptor.
        //

        // extract the path from input arguments
        let mut path = self.path(&args, "unlink")?;

        if !PathBuf::from(path.clone()).is_absolute() {
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
        }

        Ok(Operation::remove(self.file(&path).clone()))
    }

    // parse a rename line
    fn rename(&mut self, args: String) -> Result<Operation, Box<dyn std::error::Error>> {
        // int rename(const char *oldpath, const char *newpath);
        // renames  a  file,  moving it between directories if required.  Any other hard links to
        // the file (as created using link(2)) are unaffected.  Open file descriptors for oldpath
        // are also unaffected.
        //
        // Example:
        //      rename("old-path", "new-path") = 0
        //

        let (old, new) = args.split_at(
            args.find(",")
                .ok_or(Error::NotFound("= from rename line".to_string()))?,
        );

        let old = self.path(&old, "rename")?;
        let new = self.path(&new, "rename")?;

        Ok(Operation::rename(self.file(&old).clone(), new))
    }

    // parse a renameat line
    fn renameat(&mut self, args: String) -> Result<Operation, Box<dyn std::error::Error>> {
        // int renameat(int olddirfd, const char *oldpath, int newdirfd, const char *newpath);
        // renames  a  file,  moving it between directories if required.  Any other hard links to
        // the file (as created using link(2)) are unaffected.  Open file descriptors for oldpath
        // are also unaffected.
        //
        // Example:
        //      renameat2(dirfd, "o-path", dirfd, "n-path", RENAME_NOREPLACE) = 0
        //
        // If the path is absolute, then dirfd is ignored.
        // If dirfd is 'AT_FDCWD', the path is interpreted relative to the current working directory
        // of the calling process.
        // If dirfd is a file descriptor, then the path is relative to the path of the directory
        // described by the file descriptor.
        //

        let parts: Vec<&str> = args.split(",").collect();
        let dirfd1 = parts[0];
        let old = parts[1];
        let mut old = self.path(&old, "renameat")?;

        let dirfd2 = parts[2];
        let new = parts[3];
        let mut new = self.path(&new, "renameat")?;

        if !PathBuf::from(old.clone()).is_absolute() && !dirfd1.contains("AT_FDCWD") {
            // dirfd should be a valid file descriptor, so the input path is a relative path.
            old = self.relative_to_absolute(dirfd1, &old)?;
        }

        if !PathBuf::from(new.clone()).is_absolute() && !dirfd2.contains("AT_FDCWD") {
            // dirfd should be a valid file descriptor, so the input path is a relative path.
            new = self.relative_to_absolute(dirfd2, &new)?;
        }

        Ok(Operation::rename(self.file(&old).clone(), new))
    }

    // parse a getrandom line
    fn get_random(&mut self, args: String) -> Result<Operation, Box<dyn std::error::Error>> {
        // ssize_t getrandom(void *buf, size_t buflen, unsigned int flags);
        // fills the buffer pointed to by buf with up to buflen random bytes.
        //
        // Example:
        //  getrandom("a-buf", len, flags) = random_bytes_len

        let parts: Vec<&str> = args.split(",").collect();
        let _buf = parts[0].to_string();
        let len = parts[1].trim().parse::<usize>()?;
        let _flags = parts[parts.len() - 1].trim().to_string();

        Ok(Operation::get_random(len))
    }

    // parse a clone line
    fn clone(&mut self, ret: String) -> Result<Operation, Box<dyn std::error::Error>> {
        // int clone(int (*fn)(void *), void *stack, int flags, void *arg, ...
        //                  /* pid_t *parent_tid, void *tls, pid_t *child_tid */ );
        // create a new ("child") process, in a manner similar to fork(2).
        //
        // Example:
        //  clone(child_stack=0x7fb239306f30,
        //        flags=CLONE_VM|CLONE_FS|CLONE_FILES|CLONE_SIGHAND|CLONE_THREAD|CLONE_SYSVSEM|
        //        CLONE_SETTLS|CLONE_PARENT_SETTID|CLONE_CHILD_CLEARTID,
        //        parent_tid=[909193], tls=0x7fb239307700, child_tidptr=0x7fb2393079d0) = 909193

        let ret = ret.trim().parse::<usize>()?;
        Ok(Operation::clone_op(ret))
    }

    fn file(&mut self, path: &str) -> Arc<File> {
        match self.accessed_files.get(path) {
            Some(f) => f.clone(),
            None => {
                let file = Arc::new(File::new(path));
                self.accessed_files.insert(path.to_string(), file.clone());
                file
            }
        }
    }

    // extract process id, operation name, input args in-between ( and ), and return value from the input string
    fn parts(&mut self, str: &str) -> Result<Parts, Box<dyn std::error::Error>> {
        // If a system call is being executed and meanwhile another one is being called from a
        // different thread/process then strace will try to preserve the  order  of  those events
        // and mark the ongoing call as being unfinished.
        // When the call returns it will be marked as resumed.

        if str.contains("unfinished") {
            let re = Regex::new(r"^(?P<pid>\d+) (?P<remaining>.+)$")?;
            assert!(re.is_match(str));

            let cap = re.captures(str).ok_or(Error::ParseError(str.to_string()))?;

            let pid = cap["pid"].parse::<usize>()?;
            let unfinished_line = cap["remaining"].to_string();

            let unfinished_re = Regex::new(r"^(?P<op>[^\(]*)\((?P<args>.*) <unfinished ...>$")?;
            assert!(unfinished_re.is_match(&unfinished_line));

            let cap = unfinished_re
                .captures(&unfinished_line)
                .ok_or(Error::ParseError(str.to_string()))?;

            let unfinished_op = cap["op"].to_string();

            // keep the unfinished line until we see the resumed line
            self.ongoing_ops.insert(format!("{}:{}", pid, unfinished_op), unfinished_line.clone());

            return Ok(Parts::Unfinished(pid, unfinished_line));
        } else if str.contains("resumed") {
            // this is a resumed line, so extract the pid and find the corresponding unfinished
            // line in the ongoing_ops map

            let resumed_re = Regex::new(
                r"^(?P<pid>\d+)\s<... (?P<op>[^\(]+) resumed>(?P<args_remained>.*)\)\s+=\s+(?P<ret>\d+|-\d+|\?)\s*.*",
            )?;
            assert!(resumed_re.is_match(str));

            let cap = resumed_re
                .captures(str)
                .ok_or(Error::ParseError(str.to_string()))?;

            let pid = cap["pid"].parse::<usize>()?;
            let resumed_op = cap["op"].to_string();
            let ret = cap["ret"].to_string();
            let args_remained = cap["args_remained"].to_string();

            // get the unfinished line
            let unfinished_line = self
                .ongoing_ops
                .get(&format!("{}:{}", pid, resumed_op))
                .ok_or(Error::NotFound(format!("process id {}", pid)))?;

            let unfinished_re = Regex::new(r"^(?P<op>[^\(]*)\((?P<args>.*) <unfinished ...>$")?;
            assert!(unfinished_re.is_match(unfinished_line));

            let cap = unfinished_re
                .captures(unfinished_line)
                .ok_or(Error::ParseError(str.to_string()))?;

            let unfinished_op = cap["op"].to_string();
            // the operation names from both unfinished and resumed line should be the same
            assert_eq!(unfinished_op, resumed_op);

            let args = format!("{}{}", cap["args"].to_string(), args_remained);

            return Ok(Parts::Finished(pid, unfinished_op, args, ret));
        } else {
            // this is an un-interrupted operation line
            let re = Regex::new(
                r"^(?P<pid>\d+) (?P<op>[^\(]+)\((?P<args>.*)\)\s+=\s+(?P<ret>\d+|-\d+|\?)\s*.*$",
            )?;

            assert!(re.is_match(str));

            let cap = re.captures(str).ok_or(Error::ParseError(str.to_string()))?;

            return Ok(Parts::Finished(
                cap["pid"].parse::<usize>()?,
                cap["op"].to_string(),
                cap["args"].to_string(),
                cap["ret"].to_string(),
            ));
        }
    }

    // check the existance of the process id in the beginning of a line
    fn has_pid(&self, str: &str) -> Result<bool, Box<dyn std::error::Error>> {
        let re = Regex::new(r"^(?P<pid>\d+) (?P<remaining>.+)$")?;
        Ok(re.is_match(str))
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

    // convert a relative path to absolute
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
                    .ok_or(Error::NotFound(format!("= from {}", callee)))?
                    + 1,
            )
            .1
            .trim();

        // the mode is not a file or directory
        if !file_type.contains("S_IFREG") && !file_type.contains("S_IFDIR") {
            return Err(Box::new(Error::InvalidType("not file or directory".to_string())));
        }

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
                    .ok_or(Error::NotFound(format!("= from {}", callee)))?
                    + 1,
            )
            .1
            .trim()
            .parse::<usize>()?;


        if file_type.contains("S_IFREG") {
            Ok(FileDir::File(path.to_string(), file_size))
        } else {
            Ok(FileDir::Dir(path.to_string(), file_size))
        }
    }

    // get the list of existing files and directories accessed during log parsing
    pub fn existing_files(&self) -> Result<HashSet<FileDir>, Box<dyn std::error::Error>> {
        Ok(self.existing_files.clone())
    }
}

#[cfg(test)]
mod test {
    use crate::file::File;
    use crate::op::OperationType;
    use crate::parser::{Parser, Parts};
    use std::path::PathBuf;
    use std::sync::Arc;

    #[test]
    fn parts() -> Result<(), Box<dyn std::error::Error>> {
        let mut parser = Parser::new(PathBuf::new());
        let str = "909194 openat(AT_FDCWD, \"/proc/self/cgroup\", O_RDONLY|O_CLOEXEC) = 3";
        let parts = parser.parts(str.as_ref())?;
        assert_eq!(
            parts,
            Parts::Finished(
                909194,
                "openat".to_string(),
                "AT_FDCWD, \"/proc/self/cgroup\", O_RDONLY|O_CLOEXEC".to_string(),
                "3".to_string()
            )
        );

        let str = "909194 close(3)                                = 0";
        let parts = parser.parts(str.as_ref())?;
        assert_eq!(
            parts,
            Parts::Finished(
                909194,
                "close".to_string(),
                "3".to_string(),
                "0".to_string()
            )
        );

        let str =  "909194 statx(AT_FDCWD, \"a-path\", AT_STATX_SYNC_AS_STAT, STATX_ALL, 0x7ffeaceb9e30) = -1 ENOENT (No such file or directory)";
        let parts = parser.parts(str.as_ref())?;
        assert_eq!(
            parts,
            Parts::Finished(
                909194,
                "statx".to_string(),
                "AT_FDCWD, \"a-path\", AT_STATX_SYNC_AS_STAT, STATX_ALL, 0x7ffeaceb9e30"
                    .to_string(),
                "-1".to_string()
            )
        );

        let str = "909194 renameat2(AT_FDCWD, \"old_path\", AT_FDCWD, \"new_path\", RENAME_NOREPLACE) = 0";
        let parts = parser.parts(str.as_ref())?;
        assert_eq!(
            parts,
            Parts::Finished(
                909194,
                "renameat2".to_string(),
                "AT_FDCWD, \"old_path\", AT_FDCWD, \"new_path\", RENAME_NOREPLACE".to_string(),
                "0".to_string()
            )
        );

        let str = "909194 getrandom( <unfinished ...>";
        let parts = parser.parts(str.as_ref())?;
        assert_eq!(
            parts,
            Parts::Unfinished(909194, "getrandom( <unfinished ...>".to_string())
        );

        let str = "909194 <... getrandom resumed>NULL, 0, GRND_NONBLOCK) = 0";
        let parts = parser.parts(str.as_ref())?;
        assert_eq!(
            parts,
            Parts::Finished(
                909194,
                "getrandom".to_string(),
                "NULL, 0, GRND_NONBLOCK".to_string(),
                "0".to_string()
            )
        );

        let str = "909194 openat(AT_FDCWD, \"a-path\", O_WRONLY|O_CREAT|O_APPEND|O_CLOEXEC, 0666 <unfinished ...>";
        let parts = parser.parts(str.as_ref())?;
        assert_eq!(parts, Parts::Unfinished(909194, "openat(AT_FDCWD, \"a-path\", O_WRONLY|O_CREAT|O_APPEND|O_CLOEXEC, 0666 <unfinished ...>".to_string()));

        let str = "909194 <... openat resumed>)            = 5";
        let parts = parser.parts(str.as_ref())?;
        assert_eq!(
            parts,
            Parts::Finished(
                909194,
                "openat".to_string(),
                "AT_FDCWD, \"a-path\", O_WRONLY|O_CREAT|O_APPEND|O_CLOEXEC, 0666".to_string(),
                "5".to_string()
            )
        );

        let str = "909194 write(5, a-(buf, 4096 <unfinished ...>";
        let parts = parser.parts(str.as_ref())?;
        assert_eq!(
            parts,
            Parts::Unfinished(909194, "write(5, a-(buf, 4096 <unfinished ...>".to_string())
        );

        let str = "909194 <... write resumed>)             = 4096";
        let parts = parser.parts(str.as_ref())?;
        assert_eq!(
            parts,
            Parts::Finished(
                909194,
                "write".to_string(),
                "5, a-(buf, 4096".to_string(),
                "4096".to_string()
            )
        );

        Ok(())
    }

    #[test]
    fn openat() -> Result<(), Box<dyn std::error::Error>> {
        let mut parser = Parser::new(PathBuf::new());
        let line = "909193 openat(AT_FDCWD, \"a_path\", O_RDONLY|O_CLOEXEC) = 9".to_string();
        if let Parts::Finished(_, _, args, ret) = parser.parts(&line)? {
            let operations = parser.openat(args, ret)?;
            assert_eq!(operations.len(), 1);
            assert_eq!(
                operations
                    .get(0)
                    .expect("failed to read the first entry of the vector")
                    .op_type(),
                &OperationType::OpenAt(Arc::new(File::new("a_path")), 0)
            );
        } else {
            panic!("{}", format!("could not get the parts from {}", line));
        }

        let line =
            "909193 openat(AT_FDCWD, \"another_path\", O_RDONLY|O_CREAT|O_CLOEXEC, 0666) = 7"
                .to_string();
        if let Parts::Finished(_, _, args, ret) = parser.parts(&line)? {
            let operations = parser.openat(args, ret)?;
            assert_eq!(operations.len(), 2);
            assert_eq!(
                operations
                    .get(0)
                    .expect("failed to read the first entry of the vector")
                    .op_type(),
                &OperationType::Mknod(Arc::new(File::new("another_path")))
            );
            assert_eq!(
                operations
                    .get(1)
                    .expect("failed to read the second entry of the vector")
                    .op_type(),
                &OperationType::OpenAt(Arc::new(File::new("another_path")), 0)
            );
        } else {
            panic!("{}", format!("could not get the parts from {}", line));
        }

        let line =
            "909193 openat(AT_FDCWD, \"another_path\", O_RDONLY|O_CREAT|O_APPEND|O_TRUNC, 0666) = 7"
                .to_string();
        if let Parts::Finished(_, _, args, ret) = parser.parts(&line)? {
            let operations = parser.openat(args, ret)?;
            assert_eq!(operations.len(), 3);
            assert_eq!(
                operations
                    .get(0)
                    .expect("failed to read the first entry of the vector")
                    .op_type(),
                &OperationType::Mknod(Arc::new(File::new("another_path")))
            );
            assert_eq!(
                operations
                    .get(1)
                    .expect("failed to read the second entry of the vector")
                    .op_type(),
                &OperationType::Truncate(Arc::new(File::new("another_path")))
            );
            assert_eq!(
                operations
                    .get(2)
                    .expect("failed to read the third entry of the vector")
                    .op_type(),
                &OperationType::OpenAt(Arc::new(File::new("another_path")), 0)
            );
        } else {
            panic!("{}", format!("could not get the parts from {}", line));
        }

        Ok(())
    }

    #[test]
    fn read() -> Result<(), Box<dyn std::error::Error>> {
        let mut parser = Parser::new(PathBuf::new());
        let openat_line = "909196 openat(AT_FDCWD, \"a_path\", O_RDONLY|O_CLOEXEC) = 3".to_string();
        if let Parts::Finished(_, _, args, ret) = parser.parts(&openat_line)? {
            let _operation = parser.openat(args, ret)?;
        } else {
            panic!(
                "{}",
                format!("could not get the parts from {}", openat_line)
            );
        }

        let read_line1 = "909196 read(3, buf, 50) = 50".to_string();
        if let Parts::Finished(_, _, args, _) = parser.parts(&read_line1)? {
            let read_op1 = parser.read(args)?;
            assert_eq!(
                read_op1.op_type(),
                &OperationType::Read(Arc::new(File::new("a_path")), 0, 50)
            );
        } else {
            panic!("{}", format!("could not get the parts from {}", read_line1));
        }

        let read_line2 = "909196 read(3, buf, 20) = 20".to_string();
        if let Parts::Finished(_, _, args, _) = parser.parts(&read_line2)? {
            let read_op2 = parser.read(args)?;
            assert_eq!(
                read_op2.op_type(),
                &OperationType::Read(Arc::new(File::new("a_path")), 50, 20)
            );
        } else {
            panic!("{}", format!("could not get the parts from {}", read_line2));
        }

        Ok(())
    }

    #[test]
    fn pread() -> Result<(), Box<dyn std::error::Error>> {
        let mut parser = Parser::new(PathBuf::new());
        let openat_line = "909196 openat(AT_FDCWD, \"a_path\", O_RDONLY|O_CLOEXEC) = 3".to_string();
        if let Parts::Finished(_, _, args, ret) = parser.parts(&openat_line)? {
            let _operation = parser.openat(args, ret)?;
        } else {
            panic!(
                "{}",
                format!("could not get the parts from {}", openat_line)
            );
        }

        let read_line1 = "909196 pread(3, buf, 50, 100) = 50".to_string();
        if let Parts::Finished(_, _, args, _) = parser.parts(&read_line1)? {
            let pread_op1 = parser.pread(args)?;
            assert_eq!(
                pread_op1.op_type(),
                &OperationType::Read(Arc::new(File::new("a_path")), 100, 50)
            );
        } else {
            panic!("{}", format!("could not get the parts from {}", read_line1));
        }

        let read_line2 = "909196 pread(3, buf, 20, 500) = 20".to_string();
        if let Parts::Finished(_, _, args, _) = parser.parts(&read_line2)? {
            let pread_op2 = parser.pread(args)?;
            assert_eq!(
                pread_op2.op_type(),
                &OperationType::Read(Arc::new(File::new("a_path")), 500, 20)
            );
        } else {
            panic!("{}", format!("could not get the parts from {}", read_line2));
        }

        // the previous pread ops should not update the opened file offset
        let read_line3 = "909196 read(3, buf, 20) = 20".to_string();
        if let Parts::Finished(_, _, args, _) = parser.parts(&read_line3)? {
            let read_op = parser.read(args)?;
            assert_eq!(
                read_op.op_type(),
                &OperationType::Read(Arc::new(File::new("a_path")), 0, 20)
            );
        } else {
            panic!("{}", format!("could not get the parts from {}", read_line3));
        }

        Ok(())
    }

    #[test]
    fn write() -> Result<(), Box<dyn std::error::Error>> {
        let mut parser = Parser::new(PathBuf::new());
        let line = "909196 openat(AT_FDCWD, \"a_path\", O_RDONLY|O_CREAT) = 5".to_string();
        if let Parts::Finished(_, _, args, ret) = parser.parts(&line)? {
            let _operations = parser.openat(args, ret)?;
        } else {
            panic!("{}", format!("could not get the parts from {}", line));
        }

        // first write
        let write_line1 = "909196 write(5, some content here, 17) = 17".to_string();
        if let Parts::Finished(_, _, args, _) = parser.parts(&write_line1)? {
            let write_op1 = parser.write(args)?;
            assert_eq!(
                write_op1.op_type(),
                &OperationType::Write(
                    Arc::new(File::new("a_path")),
                    0,
                    17,
                    "some content here".to_string()
                )
            );
        } else {
            panic!(
                "{}",
                format!("could not get the parts from {}", write_line1)
            );
        }

        // second write
        let write_line2 = "909196 write(5, hello, 5) = 5".to_string();
        if let Parts::Finished(_, _, args, _) = parser.parts(&write_line2)? {
            let write_op2 = parser.write(args)?;
            assert_eq!(
                write_op2.op_type(),
                &OperationType::Write(Arc::new(File::new("a_path")), 17, 5, "hello".to_string())
            );
        } else {
            panic!(
                "{}",
                format!("could not get the parts from {}", write_line2)
            );
        }

        // open the file one more time to check the offset
        let line = "909196 openat(AT_FDCWD, \"a_path\", O_RDONLY|O_CREAT) = 5".to_string();
        if let Parts::Finished(_, _, args, ret) = parser.parts(&line)? {
            let _operations = parser.openat(args, ret)?;
        } else {
            panic!("{}", format!("could not get the parts from {}", line));
        }

        // now open the file with truncate flag, which should zero the size and offset
        let line = "909196 openat(AT_FDCWD, \"a_path\", O_RDONLY|O_TRUNC) = 5".to_string();
        if let Parts::Finished(_, _, args, ret) = parser.parts(&line)? {
            let _operations = parser.openat(args, ret)?;
        } else {
            panic!("{}", format!("could not get the parts from {}", line));
        }

        // write after truncate
        let write_line2 = "909196 write(5, some other content here, 10) = 10".to_string();
        if let Parts::Finished(_, _, args, _) = parser.parts(&write_line2)? {
            let write_op2 = parser.write(args)?;
            assert_eq!(
                write_op2.op_type(),
                &OperationType::Write(
                    Arc::new(File::new("a_path")),
                    0,
                    10,
                    "some other content here".to_string()
                )
            );
        } else {
            panic!(
                "{}",
                format!("could not get the parts from {}", write_line2)
            );
        }

        Ok(())
    }

    #[test]
    fn get_random() -> Result<(), Box<dyn std::error::Error>> {
        let mut parser = Parser::new(PathBuf::new());
        let line = "909196 getrandom(a_buf, 16, GRND_NONBLOCK) = 16".to_string();
        if let Parts::Finished(_, _, args, _) = parser.parts(&line)? {
            let operation = parser.get_random(args)?;
            assert_eq!(operation.op_type(), &OperationType::GetRandom(16));
        } else {
            panic!("{}", format!("could not get the parts from {}", line));
        }

        Ok(())
    }

    #[test]
    fn fstat() -> Result<(), Box<dyn std::error::Error>> {
        let mut parser = Parser::new(PathBuf::new());
        let openat_line = "909196 openat(AT_FDCWD, \"a_path\", O_RDONLY|O_CLOEXEC) = 3".to_string();
        if let Parts::Finished(_, _, args, ret) = parser.parts(&openat_line)? {
            println!("args: {}", args);
            let _operation = parser.openat(args, ret)?;
        } else {
            panic!(
                "{}",
                format!("could not get the parts from {}", openat_line)
            );
        }

        let fstat_line =
            "909196 fstat(3, {st_mode=S_IFREG|0644, st_size=95921, ...}) = 0".to_string();
        if let Parts::Finished(_, _, args, _) = parser.parts(&fstat_line)? {
            let fstat_op = parser.fstat(args)?;
            assert_eq!(
                fstat_op.op_type(),
                &OperationType::Fstat(Arc::new(File::new("a_path")))
            );

            assert_eq!(parser.existing_files.len(), 1);
            let file_size = parser
                .existing_files
                .iter()
                .next()
                .expect("failed to get the first element from hashset")
                .size();
            assert_eq!(*file_size, 95921 as usize);
        } else {
            panic!("{}", format!("could not get the parts from {}", fstat_line));
        }

        Ok(())
    }

    #[test]
    fn mkdir() -> Result<(), Box<dyn std::error::Error>> {
        let mut parser = Parser::new(PathBuf::new());

        let mkdir_line = "909196 mkdir(\"a_path\", 0777) = 0".to_string();
        if let Parts::Finished(_, _, args, _) = parser.parts(&mkdir_line)? {
            let mkdir_op = parser.mkdir(args)?;
            assert_eq!(
                mkdir_op.op_type(),
                &OperationType::Mkdir(Arc::new(File::new("a_path")), "0777".to_string())
            );
        } else {
            panic!("{}", format!("could not get the parts from {}", mkdir_line));
        }

        Ok(())
    }

    #[test]
    fn rename() -> Result<(), Box<dyn std::error::Error>> {
        let mut parser = Parser::new(PathBuf::new());
        let line = "909196 rename(\"old_path\", \"new_path\") = 0".to_string();
        if let Parts::Finished(_, _, args, _) = parser.parts(&line)? {
            let operation = parser.rename(args)?;
            assert_eq!(
                operation.op_type(),
                &OperationType::Rename(Arc::new(File::new("old_path")), "new_path".to_string())
            );
        } else {
            panic!("{}", format!("could not get the parts from {}", line));
        }

        Ok(())
    }

    #[test]
    fn renameat() -> Result<(), Box<dyn std::error::Error>> {
        let mut parser = Parser::new(PathBuf::new());
        let line =
            "909196 renameat2(AT_FDCWD, \"old_path\", AT_FDCWD, \"new_path\", RENAME_NOREPLACE) = 0"
                .to_string();
        if let Parts::Finished(_, _, args, _) = parser.parts(&line)? {
            let operation = parser.renameat(args)?;
            assert_eq!(
                operation.op_type(),
                &OperationType::Rename(Arc::new(File::new("old_path")), "new_path".to_string())
            );
        } else {
            panic!("{}", format!("could not get the parts from {}", line));
        }

        Ok(())
    }
}
