use std::{sync::Arc, time::Instant};

use crate::{ChildMetric, IntCounter, IntGauge, RegisterAction};

pub struct ActiveGauge<M>(ChildMetric<M, IntGauge>);

impl<M: 'static> ActiveGauge<M> {
    pub fn new<F: Fn(&'static M) -> &'static IntGauge>(metrics: &Arc<M>, get: F) -> Self {
        let metric = ChildMetric::create(metrics, get);
        metric.inc();
        ActiveGauge(metric)
    }
}

impl<M> Drop for ActiveGauge<M> {
    fn drop(&mut self) {
        self.0.dec();
    }
}

pub struct DurationIncMs<M> {
    start: Instant,
    count: ChildMetric<M, IntCounter>,
}

impl<M: 'static> DurationIncMs<M> {
    pub fn new<F: Fn(&'static M) -> &'static IntCounter>(metrics: &Arc<M>, get: F) -> Self {
        DurationIncMs {
            start: Instant::now(),
            count: ChildMetric::create(metrics, get),
        }
    }
}

impl<M> Drop for DurationIncMs<M> {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed().as_millis() as u64;
        self.count.shared_inc_by(elapsed as _);
    }
}

pub struct DurationIncUs<M> {
    start: Instant,
    count: ChildMetric<M, IntCounter>,
}

impl<M: 'static> DurationIncUs<M> {
    pub fn new<F: Fn(&'static M) -> &'static IntCounter>(metrics: &Arc<M>, get: F) -> Self {
        DurationIncUs {
            start: Instant::now(),
            count: ChildMetric::create(metrics, get),
        }
    }
}

impl<M> Drop for DurationIncUs<M> {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed().as_micros() as u64;
        self.count.shared_inc_by(elapsed as _);
    }
}

pub trait RegisterableMetric: 'static {
    fn register(&'static self, register: &mut RegisterAction);
}

#[derive(Default, Copy, Clone)]
pub struct NoMetrics;

impl RegisterableMetric for NoMetrics {
    fn register(&'static self, _register: &mut RegisterAction) {}
}
