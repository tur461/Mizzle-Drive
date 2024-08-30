use nix::errno::Errno;
use nix::mount::{mount, MsFlags, MntFlags};
use nix::sys::stat::Mode;
use nix::unistd::{close, ftruncate, mkfifo, write};
use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::os::unix::io::{AsRawFd, BorrowedFd, RawFd};
use std::path::Path;
use std::process::Command;
use std::io::Write;


const IMAGE_PATH: &str = "/tmp/virtual_disk.img";
const MOUNT_POINT: &str = "/tmp/virtual_disk";
const DISK_SIZE: u64 = 10 * 1024 * 1024 * 1024; // 10GB

fn create_fully_allocated_file(path: &str, size: u64) -> io::Result<()> {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(path)?;

    let fd: RawFd = file.as_raw_fd();

    // Allocate the file to the full size using ftruncate
    let borrowed_fd = BorrowedFd::borrow_raw(fd);
    unsafe {
        ftruncate(borrowed_fd, size as i64)?;
    }

    // Optionally, write a zero byte at the end to ensure space is allocated
    unsafe {
        lseek(fd, size as i64 - 1, libc::SEEK_SET)?;
        write(borrowed_fd, &[0])?;
    }

    close(fd)?;
    Ok(())
}

fn lseek(fd: RawFd, offset: i64, whence: i32) -> io::Result<i64> {
    let ret = unsafe { libc::lseek(fd, offset, whence) };
    if ret == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(ret)
    }
}

fn format_virtual_disk(path: &str) -> io::Result<()> {
    let status = Command::new("mkfs.ext4")
        .arg(path)
        .status()?;

    if !status.success() {
        return Err(io::Error::new(io::ErrorKind::Other, "mkfs.ext4 failed"));
    }

    Ok(())
}

fn mount_virtual_disk() -> nix::Result<()> {
    let source = Path::new(IMAGE_PATH);
    let target = Path::new(MOUNT_POINT);

    if !target.exists() {
        std::fs::create_dir_all(target).map_err(|e| Errno::from_i32(e.raw_os_error().unwrap_or(1)))?;
    }

    mount(Some(source), target, Some("ext4"), MsFlags::empty(), None::<&str>)?;
    Ok(())
}

fn copy_file_to_mount(source_file: &str, destination: &str) -> io::Result<()> {
    let mut source = File::open(source_file)?;
    let destination_path = Path::new(MOUNT_POINT).join(destination);
    let mut destination = OpenOptions::new()
        .write(true)
        .create(true)
        .open(destination_path)?;

    let mut buffer = vec![0; 4096];
    loop {
        let n = source.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        destination.write_all(&buffer[..n])?;
    }

    Ok(())
}

fn unmount_virtual_disk() -> nix::Result<()> {
    let target = Path::new(MOUNT_POINT);
    umount2(target, MntFlags::empty())?;
    Ok(())
}

fn main() -> io::Result<()> {
    // 1. Create a fully allocated 10GB file
    create_fully_allocated_file(IMAGE_PATH, DISK_SIZE)?;
    println!("Fully allocated 10GB virtual disk image created.");

    // 2. Format the file with ext4 filesystem
    format_virtual_disk(IMAGE_PATH)?;
    println!("Virtual disk image formatted as ext4.");

    // 3. Mount the disk image
    match mount_virtual_disk() {
        Ok(_) => println!("Virtual disk mounted."),
        Err(e) => {
            eprintln!("Failed to mount virtual disk: {:?}", e);
            return Err(io::Error::new(io::ErrorKind::Other, "Mount failed"));
        }
    }

    // 4. Transfer a file into the mounted virtual disk
    let source_file = "/path/to/your/source/file.txt"; // Change this to an actual file
    let destination = "file.txt";
    match copy_file_to_mount(source_file, destination) {
        Ok(_) => println!("File copied to virtual disk."),
        Err(e) => eprintln!("Failed to copy file: {:?}", e),
    }

    // 5. Unmount the virtual disk
    match unmount_virtual_disk() {
        Ok(_) => println!("Virtual disk unmounted."),
        Err(e) => eprintln!("Failed to unmount virtual disk: {:?}", e),
    }

    Ok(())
}

