use std::io::{BufRead, BufReader, Read};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Instant;

pub enum Event {
    Line(String),
    Done { code: Option<i32>, secs: f64 },
}

/// A running harness process, streamed line-by-line.
pub struct Runner {
    pub rx: mpsc::Receiver<Event>,
    pub cancelled: Arc<AtomicBool>,
    finished: Arc<AtomicBool>,
    pgid: i32,
}

impl Runner {
    pub fn start(
        program: &str,
        args: &[String],
        cwd: &std::path::Path,
        ctx: eframe::egui::Context,
    ) -> Result<Runner, String> {
        let mut cmd = Command::new(program);
        cmd.args(args)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Own process group so cancel kills the whole tree (harness + its children).
        cmd.process_group(0);
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("failed to spawn `{program}`: {e}"))?;
        let pgid = child.id() as i32;
        let stdout = child.stdout.take().ok_or("no stdout pipe")?;
        let stderr = child.stderr.take().ok_or("no stderr pipe")?;

        let (tx, rx) = mpsc::channel::<Event>();
        let t0 = Instant::now();

        let reader = |pipe: Box<dyn Read + Send>, tx: mpsc::Sender<Event>, ctx: eframe::egui::Context| {
            std::thread::spawn(move || {
                let mut br = BufReader::new(pipe);
                let mut buf: Vec<u8> = Vec::new();
                loop {
                    buf.clear();
                    match br.read_until(b'\n', &mut buf) {
                        Ok(0) => break,
                        Ok(_) => {
                            let line = String::from_utf8_lossy(&buf)
                                .trim_end_matches(['\n', '\r'])
                                .to_string();
                            if tx.send(Event::Line(line)).is_err() {
                                break;
                            }
                            ctx.request_repaint();
                        }
                        Err(_) => break,
                    }
                }
            })
        };
        let r1 = reader(Box::new(stdout), tx.clone(), ctx.clone());
        let r2 = reader(Box::new(stderr), tx.clone(), ctx.clone());

        let finished = Arc::new(AtomicBool::new(false));
        let finished2 = finished.clone();
        std::thread::spawn(move || {
            let _ = r1.join();
            let _ = r2.join();
            let code = child.wait().ok().and_then(|s| s.code());
            finished2.store(true, Ordering::SeqCst);
            let _ = tx.send(Event::Done {
                code,
                secs: t0.elapsed().as_secs_f64(),
            });
            ctx.request_repaint();
        });

        Ok(Runner {
            rx,
            cancelled: Arc::new(AtomicBool::new(false)),
            finished,
            pgid,
        })
    }

    /// SIGTERM the whole process group; SIGKILL if still alive after 3s.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        let pgid = self.pgid;
        unsafe {
            libc::kill(-pgid, libc::SIGTERM);
        }
        let finished = self.finished.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(3));
            if !finished.load(Ordering::SeqCst) && unsafe { libc::kill(-pgid, 0) } == 0 {
                unsafe {
                    libc::kill(-pgid, libc::SIGKILL);
                }
            }
        });
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        // Safety net: never leave an agent running after the app lets go.
        if !self.finished.load(Ordering::SeqCst) && unsafe { libc::kill(-self.pgid, 0) } == 0 {
            unsafe {
                libc::kill(-self.pgid, libc::SIGTERM);
            }
        }
    }
}
