use std::{
    io::{self, Write},
    time::SystemTime,
};

use crate::filesplit;

pub struct Logger {
    lev: log::Level,
    flspt: filesplit::FileSpliter,
    showstd: bool,
    showtm: bool,
    showfl: bool,
    showmod: bool,
    zone: Option<i32>,
}

impl Logger {
    pub fn new(ctx: &crate::Context, cg: filesplit::Config) -> Self {
        Self {
            lev: log::Level::Info,
            flspt: filesplit::FileSpliter::new(ctx, cg),
            showstd: true,
            showtm: true,
            showfl: false,
            showmod: false,
            zone: None,
        }
    }

    pub fn start(self) -> io::Result<()> {
        let flspt = self.flspt.clone();
        let lev = self.lev;
        log::set_boxed_logger(Box::new(self))
            .map_err(|e| crate::ioerr(format!("log::set_boxed_logger err:{}", e), None))?;
        log::set_max_level(lev.to_level_filter());
        std::thread::spawn(move || {
            if let Err(e) = flspt.run() {
                println!("FileSpliter run err:{}", e);
            }
        });
        Ok(())
    }

    pub fn level(&mut self, lev: log::Level) -> &mut Self {
        self.lev = lev;
        self
    }
    pub fn timezone(&mut self, o: i32) -> &mut Self {
        self.zone = Some(o);
        self
    }
    pub fn hide_stdio(&mut self) -> &mut Self {
        self.showstd = false;
        self
    }
    pub fn hide_time(&mut self) -> &mut Self {
        self.showtm = false;
        self
    }
    pub fn show_file_info(&mut self) -> &mut Self {
        self.showfl = true;
        self
    }
    pub fn show_module(&mut self) -> &mut Self {
        self.showmod = true;
        self
    }
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= self.lev
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let mut msg = format!("{}\t", record.level());
        if self.showtm {
            msg.push_str(&format!(
                " [{}]",
                match self.zone {
                    Some(v) => crate::strftime_off(SystemTime::now(), "%Y-%m-%d %H:%M:%S", v),
                    None => crate::strftime(SystemTime::now(), "%Y-%m-%d %H:%M:%S"),
                }
            ));
        }
        if self.showfl {
            if let Some(flp) = record.file() {
                msg.push_str(&format!(" [{}]", flp));
            }
        }
        if self.showmod {
            if let Some(flp) = record.module_path() {
                msg.push_str(&format!(" [{}]", flp));
            }
        }

        msg.push_str(&format!(": {} \n", record.args()));
        let bts = msg.as_bytes();
        self.flspt.push(bts);

        if self.showstd {
            let mut out = std::io::stdout().lock();
            out.write_all(bts);
        }
    }

    fn flush(&self) {
        self.flspt.flush();
    }
}
