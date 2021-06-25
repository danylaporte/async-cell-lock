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
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        Box::pin(with_deadlock_check(self.service.call(req)))
    }
}
