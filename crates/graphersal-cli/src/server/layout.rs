//! `/api/layout*`: the engine's force layout of a drawing (`graphersal_session::layout`) over
//! HTTP, in slices like the WebAssembly worker runs it: `POST /api/layout` starts a job and runs
//! its first slice, `POST /api/layout/step` runs the next, `POST /api/layout/cancel` ends it
//! keeping the positions reached so far. Every answer is the job's status; one that is no longer
//! running carries `positions` (`x, y` per vertex, rounded to 0.01). Each slice is one session
//! job of at most `budgetMs` (default [`DEFAULT_BUDGET_MS`]), so other requests (an inspector
//! click) wait at most that long, and `POST /api/cancel` (Stop) ends a running slice as
//! `cancelled`.

use std::sync::atomic::Ordering;

use graphersal_session::Session;
use graphersal_session::layout::LayoutRequest;
use serde_json::{Value, json};
use tiny_http::{Method, Request};

use super::{Answer, Context, HttpError, json_body, value};

/// The slice budget when a request names none, in milliseconds.
pub const DEFAULT_BUDGET_MS: f64 = 100.0;

fn budget(body: &Value) -> f64 {
    body.get("budgetMs")
        .and_then(Value::as_f64)
        .unwrap_or(DEFAULT_BUDGET_MS)
}

fn job_of(body: &Value) -> Result<u64, HttpError> {
    body.get("job")
        .and_then(Value::as_u64)
        .ok_or_else(|| HttpError::bad_request("expected {\"job\": <layout job>, ...}"))
}

/// The status with `positions` added when the job is no longer running.
fn with_positions(session: &mut Session, status: Value) -> Result<Value, String> {
    let mut status = status;
    if status["state"] != "running" {
        let job = status["job"].as_u64().unwrap_or_default();
        let positions: Vec<Value> = session
            .layout_positions(job)?
            .into_iter()
            .map(|v| json!((f64::from(v) * 100.0).round() / 100.0))
            .collect();
        status["positions"] = Value::Array(positions);
    }
    Ok(status)
}

/// One slice as a session job.
type SliceJob = Box<dyn FnOnce(&mut Session) -> Result<Value, String> + Send>;

pub(super) fn route(
    context: &Context,
    request: &mut Request,
    method: &Method,
    endpoint: &str,
) -> Result<Answer, HttpError> {
    if *method != Method::Post {
        return Err(HttpError(
            405,
            format!("method {method} is not allowed on /api/{endpoint}"),
        ));
    }
    let body = json_body(request)?;
    let budget_ms = budget(&body);
    // A slice counts as running: Stop (`POST /api/cancel`) ends it.
    let slice = |job: SliceJob| {
        context.running.fetch_add(1, Ordering::SeqCst);
        let answer = context.call_fallible(job);
        context.running.fetch_sub(1, Ordering::SeqCst);
        answer
    };
    match endpoint {
        "layout" => {
            let request: LayoutRequest = serde_json::from_value(body).map_err(|err| {
                HttpError::bad_request(format!(
                    "expected {{\"vertexCount\": n, \"edgeSource\": [..], \"edgeTarget\": [..], \
                     \"seed\"?, \"iterations\"?, \"budgetMs\"?}}: {err}"
                ))
            })?;
            value(slice(Box::new(move |s| {
                let status = s.layout_start(request, budget_ms)?;
                with_positions(s, status)
            }))?)
        }
        "layout/step" => {
            let job = job_of(&body)?;
            value(slice(Box::new(move |s| {
                let status = s.layout_step(job, budget_ms)?;
                with_positions(s, status)
            }))?)
        }
        "layout/cancel" => {
            let job = job_of(&body)?;
            value(context.call_fallible(move |s| {
                let status = s.layout_cancel(job)?;
                with_positions(s, status)
            })?)
        }
        _ => Err(HttpError(404, format!("unknown endpoint /api/{endpoint}"))),
    }
}
