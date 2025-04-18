// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::{
    env,
    fs,
    io::{Write, BufReader},
    io::BufRead,
    path::Path,
    os::linux::fs::MetadataExt
};
use uucore::{
    libc::{sysconf, _SC_PAGESIZE, _SC_PAGE_SIZE}, 
    error::{UResult, set_exit_code, UUsageError},
    help_about, help_usage, format_usage
};
use clap::{crate_version, ArgAction, ArgMatches, Command, Arg};

const ABOUT: &str = help_about!("mkswap.md");
const USAGE: &str = help_usage!("mkswap.md");

const SWAP_SIGNATURE: &[u8] = "SWAPSPACE2".as_bytes();
const SWAP_SIGNATURE_SZ: usize = 10;
const SWAP_VERSION: u8 = 1;


#[repr(C)]
struct SwapHeader {
    bootbits: [u8; 1024],
    version: u8,
    last_page: u32,
    nr_badpages: u32,
    uuid: [u8; 16],
    volume_name: [char; 16],
    padding: [u32; 117],
    badpages: [u32; 1]
}



pub fn mkswap(args: &ArgMatches) -> UResult<()> {


    let mut pagesize: i64 =  unsafe { sysconf(_SC_PAGESIZE)};
    if pagesize <= 0 {
        pagesize = unsafe {sysconf(_SC_PAGE_SIZE)};
        if pagesize <= 0 {
            panic!("can't determine system page size\n");
        }
    }
    assert!(pagesize > 0);


    if let Some(devstr) = args.get_one::<String>("device") {
    //let args: Vec<String> = env::args().collect();
    //let devstr = format!("{}", args[1]);

        let dev = Path::new(devstr.as_str());
        let devname = devstr.strip_prefix("/dev/").unwrap_or(&devstr);

        let mut fd = fs::File::options().create(true)
                                        .write(true)
                                        .truncate(false)
                                        .append(false)
                                        .open(dev)?;

        let stat = fd.metadata()?;

        if stat.st_uid() != 0 {
            println!("{}: {}: insecure file owner {}, fix with: chown 0:0 {}",
                uucore::util_name(), devstr, stat.st_uid(), devstr);
        }

        let devsize: u128;
    
        /*For block devices, read block size from sys/class/block */
        if stat.st_mode() == 25008 {
            let f_size = fs::File::open(format!("/sys/class/block/{devname}/size"))?;

            //horrendous but it may work, returns size in sectors
            let reader = BufReader::new(f_size);
            let vec: Vec<Result<u128, _>> = reader.lines()
                                            .map(|v| v.unwrap().parse::<u128>())
                                            .collect::<Vec<Result<u128, _>>>();
            devsize = vec[0].clone().unwrap();
            
        } else {
            devsize = (stat.st_size() as u128)/512;
            assert_eq!(stat.st_size(), stat.len());
        }
        

        let mut pagesize: i64 =  unsafe {sysconf(_SC_PAGESIZE)};
        if pagesize <= 0 {
            pagesize = unsafe {sysconf(_SC_PAGE_SIZE)};
            if pagesize <= 0 {
                pagesize = stat.st_blksize() as i64;
                if pagesize <= 0 {
                    pagesize = 4096;
                }
            }
        }
        
        assert!(pagesize > 0);
        assert!(devsize > 0);

        let pages = (devsize*512) / pagesize as u128;
        let lastpage = pages - 1;

        if pages < 10 {
            println!("swap space needs to be at least {}KiB",
                    10 * pagesize / 1024);
            return Ok(());
        }

        assert!(pages > 0);
        assert!(lastpage > 0);
        debug_assert_eq!(pages, ((devsize*512)/4096));

        
        //allocate signature page
        let mut buf = Box::<[u8]>::new_uninit_slice(pagesize as usize);
        

        unsafe {
            //initialize the page
            buf.as_mut_ptr().write_bytes(0, pagesize as usize); 
        

            //fill up swap header identically to C
            let swap_hdr = buf.as_mut_ptr() as *mut SwapHeader; 
            (*swap_hdr).version = SWAP_VERSION;
            (*swap_hdr).last_page = lastpage as u32;
            //(*swap_hdr).uuid = *Uuid::new_v4().as_bytes();    
        }
            
        let mut buf = unsafe {buf.assume_init()};

        /* Swap signature */
        let _ = &buf[(pagesize as usize -SWAP_SIGNATURE_SZ)..pagesize as usize].copy_from_slice(SWAP_SIGNATURE);


        fd.write_all(&buf)?;
        fd.flush()?;
        fd.sync_all()?;

        println!("Setting up swapspace version 1, size = {}KiB", (((pages-1) * pagesize as u128) / 1024));


    } else {
        return Err(UUsageError::new(1, format!("Error: no device specified\nUsage: {} -d device", uucore::util_name())));
    }

    Ok(())
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uu_app().try_get_matches_from(args)?;
    if let Err(e) = mkswap(&matches) {
        set_exit_code(2);
        uucore::show_error!("{}", e);
    };
    Ok(())
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new("device")
                .short('d')
                .long("device")
                .action(ArgAction::Set)
                .help("block device")
                
        )
        
}
