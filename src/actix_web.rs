use crate::with_deadlock_check;
use actix_web_04::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    Error,
};
use std::{
    future::{ready, Future, Ready},
    pin::Pin,
    task::{Context, Poll},
};

pub struct DeadlockDetector;

impl<S, B> Transform<S, ServiceRequest> for DeadlockDetector
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = DeadlockDetectorMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(DeadlockDetectorMiddleware { service }))
    }
}

#[doc(hidden)]
pub struct DeadlockDetectorMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for DeadlockDetectorMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<Result<Self::Response, Self::Error>>;

    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let route = req.match_pattern().unwrap_or_else(|| "default".into());
        let method = req.method().as_str().to_string();
        let task_name = format!("{method} {route}");

        #[cfg(feature = "telemetry")]
        let active_gauge = metrics::gauge!(
            "active_http_req_in_gauge",
            "route" => route.clone(),
            "method" => method.clone()
        );

        let f = self.service.call(req);

        Box::pin(async move {
            #[cfg(feature = "telemetry")]
            metrics::counter!(
                "http_req_in_counter",
                "route" => route.clone(),
                "method" => method.clone()
            )
            .increment(1);

            #[cfg(feature = "telemetry")]
            let complete = metrics::counter!("http_req_in_completed_count", "route" => route, "method" => method);

            #[cfg(feature = "telemetry")]
            let _active = crate::monitors::ActiveGauge::new(active_gauge);

            #[cfg(feature = "telemetry")]
            let _complete = crate::monitors::CountOnEnd(complete);

            with_deadlock_check(f, task_name).await
        })
    }
}

type LocalBoxFuture<T> = Pin<Box<dyn Future<Output = T>>>;
