use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use crate::error::Error;
use crate::ops::Operation;

pub struct Parser {
    log_file: PathBuf,
    fd_map: HashMap<i32, OpenedFile>
}

#[derive(Debug)]
struct OpenedFile {
    path: String,
    offset: i32,
    size: usize
}

impl OpenedFile {
    pub fn new(path: String, offset: i32, size: usize) -> Self {
        OpenedFile {
            path,
            offset,
            size
        }
    }
}

impl Parser {
    pub fn new(log_file: PathBuf) -> Self {
        Parser {
            log_file,
            fd_map: HashMap::new()
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
                line.starts_with("readlink") // readlink op
            {
                continue;
            }

            if line.starts_with("openat") {
                let operation = self.openat(line.as_ref())?;
                operations.push(operation);
            }

            if line.starts_with("read") {
                // read op updates the file offset
                operations.push(self.read(line.as_ref())?);
            }

            if line.starts_with("pread") {
                // pread op does not update the file offset
                operations.push(self.pread(line.as_ref())?);
            }

            if line.starts_with("getrandom") {
                operations.push(self.get_random(line.as_ref())?);
            }

            if line.starts_with("write") {
                operations.push(self.write(line.as_ref())?);
            }

            if line.starts_with("mkdir") {
                operations.push(self.mkdir(line.as_ref())?);
            }
        }

        Ok(operations)
    }

    // parse an openat line
    fn openat(&mut self, line: &str) -> Result<Operation, Box<dyn std::error::Error>> {

        // an openat line of the strace log is like:
        //  openat(dirfd, "a-path", flags) = fd

        // replace single and double quotes and spaces
        let line = line
            .replace(" ", "")
            .replace("\"", "")
            .replace("\'", "");

        let parts: Vec<&str> = line.split("=").collect();
        let op = parts[0]; // the command body
        let fd = parts[1].parse::<i32>()?; // the file descriptor after '='

        let parts: Vec<&str> = op.split(",").collect();
        let _dirfd = parts[0];
        let path = parts[1].to_string();
        let flags = parts[parts.len() - 1];

        let (offset, size)  = match self.fd_map.get(&fd) {
            Some(of) => {
                // we have already created an OpenAt operation
                if flags.contains("O_TRUNC") {
                    (0, 0) // the file is opened in truncate mode, so the offset should be 0
                } else  {
                    (of.offset, of.size) // the file is opened in non-truncate mode (e.g, append mode), so the offset is the same
                }
            },
            None => {
                // the file is opened for the first time
                (0, 0)
            }
        };

        self.fd_map.insert(fd, OpenedFile::new(path.clone(), offset, size));

        let operation  = if flags.contains("O_CREAT") {
            Operation::mknod(
                size,
                offset,
                path,
            )
        } else {
            Operation::open_at(
                offset,
                path,
            )
        };

        Ok(operation)
    }

    // parse a read line
    fn read(&mut self, line: &str) -> Result<Operation, Box<dyn std::error::Error>> {

        // a read line of the strace log is like:
        //  read(fd, "a-buf", len) = read_len
        //
        // this operation reads len bytes from opened file offset and changes the opened file offset after read

        // replace single and double quotes and spaces
        let line = line
            .replace(" ", "")
            .replace("\"", "")
            .replace("\'", "");

        let body = line.split_at(line.rfind("=").ok_or(Error::NotFound("=".to_string()))?).0;

        // extract the read arguments between '(' and ')'
        let args = body.split_at(body.find("(").ok_or(Error::NotFound("(".to_string()))? + 1).1;
        let args = args.split_at(args.rfind(")").ok_or(Error::NotFound(")".to_string()))?).0;

        let parts: Vec<&str> = args.split(",").collect();
        let fd = parts[0].parse::<i32>()?;
        let _buf = parts[1].to_string();
        let len = parts[parts.len() - 1].parse::<usize>()?;

        // find the read path based on the file descriptor
        let opend_file = self.fd_map.get(&fd)
            .ok_or(Error::NotFound(format!("file descriptor {}", fd)))?;

        let path = opend_file.path.clone();
        let offset = opend_file.offset;
        let size = opend_file.size;

        // update the offset of the opened file in the fd_map
        self.fd_map.insert(fd, OpenedFile::new(path.clone(), offset + len as i32, size));

        Ok(Operation::read(len, offset, path))
    }

    // parse a read line
    fn pread(&mut self, line: &str) -> Result<Operation, Box<dyn std::error::Error>> {

        // a pread (or pread64) line of the strace log is like:
        //  pread(fd, "a-buf", len, offset) = read_len
        //
        // the operation reads len bytes from input offset and does not change the opened file offset after read

        // replace single and double quotes and spaces
        let line = line
            .replace(" ", "")
            .replace("\"", "")
            .replace("\'", "");

        let body = line.split_at(line.rfind("=").ok_or(Error::NotFound("=".to_string()))?).0;
        // the bytes read is after '='

        // extract the read arguments between '(' and ')'
        let args = body.split_at(body.find("(").ok_or(Error::NotFound("(".to_string()))? + 1).1;
        let args = args.split_at(args.rfind(")").ok_or(Error::NotFound(")".to_string()))?).0;

        let parts: Vec<&str> = args.split(",").collect();
        let fd = parts[0].parse::<i32>()?;
        let _buf = parts[1].to_string();
        let len = parts[2].parse::<usize>()?;
        let offset = parts[parts.len() - 1].parse::<i32>()?;

        // find the read path based on the file descriptor
        let opend_file = self.fd_map.get(&fd)
            .ok_or(Error::NotFound(format!("file descriptor {}", fd)))?;

        Ok(Operation::read(len, offset, opend_file.path.clone()))
    }

    // parse a write line
    fn write(&mut self, line: &str) -> Result<Operation, Box<dyn std::error::Error>> {

        // a write line of the strace log is like:
        //  write(fd, "a-string", len) = write_len

        // replace single and double quotes and spaces
        let line = line
            .replace("\'", "");

        let body = line.split_at(line.rfind("=").ok_or(Error::NotFound("=".to_string()))?).0;
        // the bytes written is after '='

        // extract the read arguments between '(' and ')'
        let args = body.split_at(body.find("(").ok_or(Error::NotFound("(".to_string()))? + 1).1;
        let args = args.split_at(args.rfind(")").ok_or(Error::NotFound(")".to_string()))?).0;

        let parts: Vec<&str> = args.split(",").collect();
        let fd = parts[0].parse::<i32>()?;
        let content = parts[1].trim().to_string();
        let len = parts[parts.len() - 1].trim().parse::<usize>()?;

        if fd == 0 || fd == 1 || fd ==2 {
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
                self.fd_map.insert(fd, OpenedFile::new(path, offset + len as i32, size + len));

                Ok(op)
            },
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

        // replace single and double quotes and spaces
        let line = line
            .replace("\'", "");

        let body = line.split_at(line.rfind("=").ok_or(Error::NotFound("=".to_string()))?).0;
        // the error code is after '='

        // extract the read arguments between '(' and ')'
        let args = body.split_at(body.find("(").ok_or(Error::NotFound("(".to_string()))? + 1).1;
        let args = args.split_at(args.rfind(")").ok_or(Error::NotFound(")".to_string()))?).0;

        let parts: Vec<&str> = args.split(",").collect();
        let path = parts[0].to_string();
        let mode = parts[1].trim().to_string();

        Ok(Operation::mkdir(mode, path))
    }

    // parse a getrandom line
    fn get_random(&mut self, line: &str) -> Result<Operation, Box<dyn std::error::Error>> {

        // a getrandom line of the strace log is like:
        //  getrandom("a-buf", len, flags) = random_bytes_len

        // replace single and double quotes and spaces
        let line = line
            .replace(" ", "")
            .replace("\"", "")
            .replace("\'", "");

        let body = line.split_at(line.rfind("=").ok_or(Error::NotFound("=".to_string()))?).0;
        // the number of random bytes generated is after '='

        // extract the read arguments between '(' and ')'
        let args = body.split_at(body.find("(").ok_or(Error::NotFound("(".to_string()))? + 1).1;
        let args = args.split_at(args.rfind(")").ok_or(Error::NotFound(")".to_string()))?).0;

        let parts: Vec<&str> = args.split(",").collect();
        let _buf = parts[0].to_string();
        let len = parts[1].parse::<usize>()?;
        let _flags = parts[parts.len() - 1].to_string();


        Ok(Operation::get_random(len))
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;
    use crate::error::Error;
    use crate::ops::OperationType;
    use crate::parser::Parser;

    #[test]
    fn openat() -> Result<(), Error> {
        let mut parser = Parser::new(PathBuf::new());
        let line = "openat(AT_FDCWD, 'a_path', O_RDONLY|O_CLOEXEC) = 9".to_string();
        let operation = parser.openat(line.as_ref())?;
        assert_eq!(operation.kind, OperationType::OpenAt);
        assert_eq!(operation.path.expect("failed to extract the path"), "a_path");
        assert!(operation.size.is_none());
        assert_eq!(operation.offset.expect("failed to extract the offset"), 0);


        let line = "openat(AT_FDCWD, 'another_path', O_RDONLY|O_CREAT|O_CLOEXEC) = 7".to_string();
        let operation = parser.openat(line.as_ref())?;
        assert_eq!(operation.kind, OperationType::Mknod);
        assert_eq!(operation.path.expect("failed to extract the path"), "another_path");
        assert_eq!(operation.size.expect("failed to extract the size"), 0);
        assert_eq!(operation.offset.expect("failed to extract the offset"), 0);

        Ok(())
    }

    #[test]
    fn read() -> Result<(), Error> {

        let mut parser = Parser::new(PathBuf::new());
        let openat_line = "openat(AT_FDCWD, 'a_path', O_RDONLY|O_CLOEXEC) = 3".to_string();
        let _operation = parser.openat(openat_line.as_ref())?;

        let read_line1 = "read(3, buf, 50) = 50".to_string();
        let read_op1 = parser.read(read_line1.as_ref())?;
        assert_eq!(read_op1.kind, OperationType::Read);
        assert!(read_op1.offset.is_some());
        assert_eq!(read_op1.offset.expect("failed to extract the offset"), 0);
        assert_eq!(read_op1.path.expect("failed to extract the path"), "a_path");
        assert!(read_op1.size.is_some());
        assert_eq!(read_op1.size.expect("failed to extract the size"), 50);

        let read_line2 = "read(3, buf, 20) = 20".to_string();
        let read_op2 = parser.read(read_line2.as_ref())?;
        assert_eq!(read_op2.kind, OperationType::Read);
        assert!(read_op2.offset.is_some());
        assert_eq!(read_op2.offset.expect("failed to extract the offset"), 50);
        assert_eq!(read_op2.path.expect("failed to extract the path"), "a_path");
        assert_eq!(read_op2.size.expect("failed to extract the size"), 20);

        Ok(())
    }

    #[test]
    fn pread() -> Result<(), Error> {

        let mut parser = Parser::new(PathBuf::new());
        let openat_line = "openat(AT_FDCWD, 'a_path', O_RDONLY|O_CLOEXEC) = 3".to_string();
        let _operation = parser.openat(openat_line.as_ref())?;

        let read_line1 = "pread(3, buf, 50, 100) = 50".to_string();
        let pread_op1 = parser.pread(read_line1.as_ref())?;
        assert_eq!(pread_op1.kind, OperationType::Read);
        assert!(pread_op1.offset.is_some());
        assert_eq!(pread_op1.offset.expect("failed to extract the offset"), 100);
        assert_eq!(pread_op1.path.expect("failed to extract the path"), "a_path");
        assert_eq!(pread_op1.size.expect("failed to extract the size"), 50);

        let read_line2 = "pread(3, buf, 20, 500) = 20".to_string();
        let pread_op2 = parser.pread(read_line2.as_ref())?;
        assert_eq!(pread_op2.kind, OperationType::Read);
        assert_eq!(pread_op2.offset.expect("failed to extract the offset"), 500);
        assert_eq!(pread_op2.path.expect("failed to extract the path"), "a_path");
        assert_eq!(pread_op2.size.expect("failed to extract the size"), 20);


        // the previous pread ops should not update the opened file offset
        let read_line3 = "read(3, buf, 20) = 20".to_string();
        let read_op = parser.read(read_line3.as_ref())?;
        assert_eq!(read_op.kind, OperationType::Read);
        assert_eq!(read_op.offset.expect("failed to extract the offset"), 0);
        assert_eq!(read_op.path.expect("failed to extract the path"), "a_path");
        assert_eq!(read_op.size.expect("failed to extract the size"), 20);

        Ok(())
    }

    #[test]
    fn write() -> Result<(), Error> {

        let mut parser = Parser::new(PathBuf::new());
        let line = "openat(AT_FDCWD, 'a_path', O_RDONLY|O_CREAT) = 5".to_string();
        let operation = parser.openat(line.as_ref())?;
        assert_eq!(operation.kind, OperationType::Mknod);
        assert_eq!(operation.path.expect("failed to extract the path"), "a_path");
        assert_eq!(operation.size.expect("failed to extract the size"), 0);
        assert_eq!(operation.offset.expect("failed to extract the offset"), 0);

        // first write
        let write_line1 = "write(5, 'some content here', 17) = 17".to_string();
        let write_op1 = parser.write(write_line1.as_ref())?;
        assert_eq!(write_op1.kind, OperationType::Write("some content here".to_string()));
        assert_eq!(write_op1.path.expect("failed to extract the path"), "a_path");
        assert_eq!(write_op1.size.expect("failed to extract the size"), 17);
        assert_eq!(write_op1.offset.expect("failed to extract the offset"), 0);

        // second write
        let write_line2 = "write(5, 'hello', 5) = 5".to_string();
        let write_op2 = parser.write(write_line2.as_ref())?;
        assert_eq!(write_op2.kind, OperationType::Write("hello".to_string()));
        assert_eq!(write_op2.path.expect("failed to extract the path"), "a_path");
        assert_eq!(write_op2.size.expect("failed to extract the size"), 5);
        assert_eq!(write_op2.offset.expect("failed to extract the offset"), 17);


        // open the file one more time to check the offset
        let line = "openat(AT_FDCWD, 'a_path', O_RDONLY|O_CREAT) = 5".to_string();
        let operation = parser.openat(line.as_ref())?;
        assert_eq!(operation.kind, OperationType::Mknod);
        assert_eq!(operation.path.expect("failed to extract the path"), "a_path");
        assert_eq!(operation.size.expect("failed to extract the size"), 17 + 5);
        assert_eq!(operation.offset.expect("failed to extract the offset"), 17 + 5);

        // now open the file with truncate flag, which should zero the size and offset
        let line = "openat(AT_FDCWD, 'a_path', O_RDONLY|O_CREAT|O_TRUNC) = 5".to_string();
        let operation = parser.openat(line.as_ref())?;
        assert_eq!(operation.kind, OperationType::Mknod);
        assert_eq!(operation.path.expect("failed to extract the path"), "a_path");
        assert_eq!(operation.size.expect("failed to extract the size"), 0);
        assert_eq!(operation.offset.expect("failed to extract the offset"), 0);

        // write after truncate
        let write_line2 = "write(5, 'some other content here', 10) = 10".to_string();
        let write_op2 = parser.write(write_line2.as_ref())?;
        assert_eq!(write_op2.kind, OperationType::Write("some other content here".to_string()));
        assert_eq!(write_op2.path.expect("failed to extract the path"), "a_path");
        assert_eq!(write_op2.size.expect("failed to extract the size"), 10);
        assert_eq!(write_op2.offset.expect("failed to extract the offset"), 0);

        Ok(())
    }

    #[test]
    fn get_random() -> Result<(), Error> {
        let mut parser = Parser::new(PathBuf::new());
        let line = "getrandom('a_buf', 16, GRND_NONBLOCK) = 16".to_string();
        let operation = parser.get_random(line.as_ref())?;
        assert_eq!(operation.kind, OperationType::GetRandom);
        assert_eq!(operation.size.expect("failed to extract size"), 16);

        Ok(())
    }
}