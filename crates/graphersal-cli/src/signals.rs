//! SIGINT and SIGTERM (Ctrl+C, `kill`): a Store open in this process is closed cleanly before
//! the process exits, so its next open reports a clean close and replays nothing it does not
//! have to. A second signal while the store closes exits at once.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use graphersal::exec::CancelToken;
use graphersal::persist::Store;

/// The exit code after an interruption (128 + SIGINT, as shells report it).
pub const EXIT_INTERRUPTED: i32 = 130;

/// Installs the handler: the first signal cancels the running query (`cancel`), closes `store`
/// and exits with `exit_code`; a second one exits with [`EXIT_INTERRUPTED`] immediately.
/// Only one handler per process; a failure to install it is a warning.
pub fn close_store_on_signal(store: Arc<Store>, cancel: Option<CancelToken>, exit_code: i32) {
    let signals = AtomicUsize::new(0);
    let installed = ctrlc::set_handler(move || {
        if signals.fetch_add(1, Ordering::SeqCst) > 0 {
            errln!("Interrupted again: exiting without waiting for the store to close.");
            std::process::exit(EXIT_INTERRUPTED);
        }
        if let Some(cancel) = &cancel {
            cancel.cancel();
        }
        // The close waits for a running commit; the handler thread stays free for a second
        // signal meanwhile.
        let store = store.clone();
        let closing = std::thread::Builder::new()
            .name("graphersal-close".into())
            .spawn(move || {
                errln!(
                    "Stopping: closing the store {} (Ctrl+C again to exit at once)...",
                    store.location()
                );
                match store.shutdown() {
                    Ok(()) if store.maintenance().is_some() => {
                        errln!(
                            "The store {} was left as it was (maintenance mode).",
                            store.location()
                        );
                    }
                    Ok(()) => errln!("The store {} was closed cleanly.", store.location()),
                    Err(err) => errln!("{}", graphersal::error::format_diagnostic(&err)),
                }
                std::process::exit(exit_code);
            });
        if closing.is_err() {
            std::process::exit(EXIT_INTERRUPTED);
        }
    });
    if let Err(err) = installed {
        errln!("warning: cannot handle Ctrl+C ({err}); stop with /exit to close the store cleanly");
    }
}
