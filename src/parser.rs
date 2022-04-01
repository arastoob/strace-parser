use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use crate::error::Error;
use crate::ops::{Operation, OperationType};

pub struct Parser {
    log_file: PathBuf,
    fd_map: HashMap<i32, String>
}

impl Parser {
    pub fn new(log_file: PathBuf) -> Self {
        Parser {
            log_file,
            fd_map: HashMap::new()
        }
    }

    pub fn parse(&mut self) -> Result<Vec<Operation>, Error> {

        let mut operations = vec![];

        let file = File::open(self.log_file.clone())?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line?;

            // filter out the operations with error result
            if line.contains("= -1") {
                continue;
            }

            if line.starts_with("openat") {
                let operation = self.openat_line(line.as_ref())?;
                operations.push(operation);
            }

            if line.starts_with("read") {
                operations.push(self.read_line(line.as_ref())?);
            }

            if line.starts_with("close") {
                self.close_line(line.as_ref())?;
            }
        }

        Ok(vec![])
    }

    // parse an openat line
    fn openat_line(&mut self, line: &str) -> Result<Operation, Error> {

        // an openat line of the strace log is like:
        //  openat(dirfd, "a-path", flags) = fd

        // replace single and double quotes and spaces
        let line = line
            .replace(" ", "")
            .replace("\"", "")
            .replace("\'", "");

        let parts: Vec<&str> = line.split("=").collect();
        let op = parts[0]; // the command body
        let fd = parts[1].parse::<i32>().unwrap(); // the file descriptor after '='

        let parts: Vec<&str> = op.split(",").collect();
        let _dirfd = parts[0];
        let path = parts[1].to_string();
        let flags = parts[2];

        self.fd_map.insert(fd, path.clone());

        let operation  = if flags.contains("O_CREAT") {
            Operation::new(
                OperationType::Mknod,
                None,
                path,
            )
        } else {
            Operation::new(
                OperationType::OpenAt,
                None,
                path,
            )
        };

        Ok(operation)
    }

    // parse a read line
    fn read_line(&self, line: &str) -> Result<Operation, Error> {

        // a read line of the strace log is like:
        //  read(fd, "a-buf", len) = read_len

        // replace single and double quotes and spaces
        let line = line
            .replace(" ", "")
            .replace("\"", "")
            .replace("\'", "");

        let parts: Vec<&str> = line.split("=").collect();
        let body = parts[0]; // the command body
        // the bytes read is after '='

        // extract the read arguments between '(' and ')'
        let args = body.split_at(body.rfind("(").unwrap() + 1).1;
        let args = args.split_at(args.find(")").unwrap()).0;

        let parts: Vec<&str> = args.split(",").collect();
        let fd = parts[0].parse::<i32>().unwrap();
        let _buf = parts[1].to_string();
        let len = parts[2].parse::<usize>().unwrap();

        // find the read path based on the file descriptor
        let path = self.fd_map.get(&fd)
            .ok_or(Error::NotFound(format!("file descriptor {}", fd)))?.to_string();

        Ok(Operation {
            kind: OperationType::Read,
            size: Some(len),
            path
        })
    }

    // parse a close line
    fn close_line(&mut self, line: &str) -> Result<(), Error> {

        // a close line of the strace log is like:
        //  close(fd)

        // replace single and double quotes and spaces
        let line = line
            .replace(" ", "")
            .replace("\"", "")
            .replace("\'", "");


        // extract the close arguments between '(' and ')', which is a file descriptor
        let arg = line.split_at(line.rfind("(").unwrap() + 1).1;
        let fd = arg.split_at(arg.find(")").unwrap()).0;

        let fd = fd.parse::<i32>().unwrap();

        // we have reached a close line, so we should remove the fd from the fd_map
        self.fd_map.remove(&fd)
            .ok_or(Error::NotFound(format!("file descriptor {}", fd)))?.to_string();

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;
    use crate::error::Error;
    use crate::ops::OperationType;
    use crate::parser::Parser;

    #[test]
    fn openat_line() -> Result<(), Error> {
        let mut parser = Parser::new(PathBuf::new());
        let line = "openat(AT_FDCWD, 'a_path', O_RDONLY|O_CLOEXEC) = 9".to_string();
        let operation = parser.openat_line(line.as_ref())?;
        assert_eq!(operation.kind, OperationType::OpenAt);
        assert_eq!(operation.path, "a_path");
        assert!(operation.size.is_none());


        let line = "openat(AT_FDCWD, 'another_path', O_RDONLY|O_CREAT|O_CLOEXEC) = 7".to_string();
        let operation = parser.openat_line(line.as_ref())?;
        assert_eq!(operation.kind, OperationType::Mknod);
        assert_eq!(operation.path, "another_path");
        assert!(operation.size.is_none());

        Ok(())
    }

    #[test]
    fn read_line() -> Result<(), Error> {

        let mut parser = Parser::new(PathBuf::new());
        let openat_line = "openat(AT_FDCWD, 'a_path', O_RDONLY|O_CLOEXEC) = 3".to_string();
        let operation = parser.openat_line(openat_line.as_ref())?;

        let read_line = "read(3, buf, 832) = 832".to_string();
        let read_op = parser.read_line(read_line.as_ref())?;
        assert_eq!(read_op.kind, OperationType::Read);
        assert_eq!(read_op.path, "a_path");
        assert!(read_op.size.is_some());
        assert_eq!(read_op.size.unwrap(), 832);

        Ok(())
    }

    #[test]
    fn close_line() -> Result<(), Error> {

        let mut parser = Parser::new(PathBuf::new());
        let openat_line = "openat(AT_FDCWD, 'a_path', O_RDONLY|O_CLOEXEC) = 3".to_string();
        let operation = parser.openat_line(openat_line.as_ref())?;

        assert!(!parser.fd_map.is_empty());

        let close_line = "close(3)".to_string();
        parser.close_line(close_line.as_ref())?;
        assert!(parser.fd_map.is_empty());

        Ok(())
    }
}