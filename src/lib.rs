use std::{any::Any, borrow::Cow, fmt::Display, ops::Deref, sync::{atomic::{AtomicU64, Ordering}, Arc}};

use helpers::RegisterableMetric;

#[derive(Default, Debug)]
pub struct IntCounter(AtomicU64);

#[derive(Default, Debug)]
pub struct IntGauge(AtomicU64);

pub mod helpers;

pub struct ChildMetric<T, C: 'static> {
    arc: Arc<T>,
    child: &'static C,
}

impl<T, C: 'static> Deref for ChildMetric<T, C> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        self.child
    }
}

impl<T: 'static, C: 'static> Clone for ChildMetric<T, C> {
    fn clone(&self) -> Self {
        Self {
            arc: self.arc.clone(),
            child: self.child,
        }
    }
}

impl<T: 'static, C: 'static> ChildMetric<T, C> {
    pub fn create<F: Fn(&'static T) -> &'static C>(arc: &Arc<T>, get: F) -> Self {
        let cloned = arc.clone();
        let item = get(unsafe { std::mem::transmute::<&T, &'static T>(&cloned) });
        Self {
            arc: cloned,
            child: item,
        }
    }
}

impl IntCounter {
    pub fn owned_inc(&self) {
        self.owned_inc_by(1);
    }

    pub fn inc(&self) {
        self.shared_inc();
    }

    pub fn inc_by(&self, amount: u64) {
        self.shared_inc_by(amount);
    }

    pub fn shared_inc(&self) {
        self.shared_inc_by(1);
    }

    pub fn owned_inc_by(&self, amount: u64) {
        self.0.fetch_add(amount, Ordering::Relaxed);
    }

    pub fn shared_inc_by(&self, amount: u64) {
        self.0.fetch_add(amount, Ordering::AcqRel);
    }
}

impl IntGauge {
    pub fn set(&self, value: u64) {
        self.0.store(value, Ordering::Relaxed);
    }

    pub fn owned_dec(&self) {
        self.owned_dec_by(1);
    }

    pub fn dec(&self) {
        self.shared_dec();
    }

    pub fn shared_dec(&self) {
        self.shared_dec_by(1);
    }

    pub fn owned_dec_by(&self, amount: u64) {
        self.0.fetch_sub(amount, Ordering::Relaxed);
    }

    pub fn shared_dec_by(&self, amount: u64) {
        self.0.fetch_sub(amount, Ordering::AcqRel);
    }

    pub fn inc(&self) {
        self.shared_inc();
    }

    pub fn shared_inc(&self) {
        self.shared_inc_by(1);
    }

    pub fn owned_inc_by(&self, amount: u64) {
        self.0.fetch_add(amount, Ordering::Relaxed);
    }

    pub fn shared_inc_by(&self, amount: u64) {
        self.0.fetch_add(amount, Ordering::AcqRel);
    }
}

pub struct PromMetricRegistry {
    /* note: keep reference to Arc to ensure it doesn't drop */
    metric_holders: Vec<Arc<dyn Any>>,
    metrics: Vec<RegisteredMetric>,
    base_attributes: Vec<[Cow<'static, str>; 2]>,
}

impl Default for PromMetricRegistry {
    fn default() -> Self {
        let base_attributes = if let Some(details) = pkg_details::try_get() {
            vec![
                [Cow::Borrowed("program"), Cow::Borrowed(details.pkg_name)],
                [Cow::Borrowed("pkg_version"), Cow::Borrowed(details.pkg_version)],
            ]
        } else {
            Vec::new()
        };

        PromMetricRegistry {
            metric_holders: Vec::new(),
            metrics: Vec::new(),
            base_attributes,
        }
    }
}

unsafe impl Send for PromMetricRegistry {}
unsafe impl Sync for PromMetricRegistry {}

struct RegisteredMetric {
    metric_type: MetricType,
    name: Cow<'static, str>,
    value: &'static AtomicU64,
    attributes: Vec<[Cow<'static, str>; 2]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MetricType {
    IntCounter,
    IntGauge,
}

impl Display for MetricType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IntCounter => write!(f, "counter"),
            Self::IntGauge => write!(f, "gauge"),
        }
    }
}

impl Display for PromMetricRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut last = None;

        for metric in &self.metrics {
            let matches = if let Some((last, ty)) = &last {
                last == &metric.name && *ty == metric.metric_type
            } else {
                false
            };

            if !matches {
                writeln!(f, "# HELP {}", metric.name)?;
                writeln!(f, "# TYPE {} {}", metric.name, metric.metric_type)?;
                last = Some((metric.name.clone(), metric.metric_type));
            }
            write!(f, "{}", metric.name)?;
            let end = metric.attributes.len();
            for (i, [key, value]) in metric.attributes.iter().enumerate() {
                if i == 0 {
                    write!(f, "{{{}=\"{}\"", key, value)?;
                    if end == 1 {
                        write!(f, "}}")?;
                    }
                }
                else if i + 1 == end {
                    write!(f, ",{}=\"{}\"}}", key, value)?;
                }
                else {
                    write!(f, ",{}=\"{}\"", key, value)?;
                }
            }
            
            writeln!(f, " {}", metric.value.load(Ordering::Relaxed))?;
        }

        Ok(())
    }
}

impl PromMetricRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<M: RegisterableMetric + 'static>(&mut self, metrics: &Arc<M>) {
        self.register_fn(metrics, |m, reg| {
            m.register(reg);
        });
    }

    pub fn register_fn<'a, T: 'static>(&'a mut self, metrics: &Arc<T>, register: impl FnOnce(&'static T, &mut RegisterAction<'a>)) {
        /* allows us to keep static references as we own an Arc copy */
        self.metric_holders.push(Arc::clone(metrics) as Arc<dyn Any>);

        let mut action = RegisterAction {
            name_prefix: None,
            metrics: &mut self.metrics,
            base_attributes: self.base_attributes.clone(),
        };

        let metric_ref = unsafe { std::mem::transmute::<&T, &'static T>(metrics) };
        register(metric_ref, &mut action);
    }
}

pub struct RegisterAction<'a> {
    metrics: &'a mut Vec<RegisteredMetric>,
    name_prefix: Option<String>,
    base_attributes: Vec<[Cow<'static, str>; 2]>,
}

impl RegisterAction<'_> {
    pub fn child(&mut self) -> RegisterAction {
        RegisterAction {
            metrics: self.metrics,
            name_prefix: self.name_prefix.clone(),
            base_attributes: self.base_attributes.clone(),
        }
    }

    pub fn name_prefix<S: Into<String>>(&mut self, prefix: S) -> &mut Self {
        self.name_prefix = Some(prefix.into());
        self
    }

    pub fn base_attr<K: Into<Cow<'static, str>>, V: Into<Cow<'static, str>>>(&mut self, key: K, value: V) -> &mut Self {
        let key = key.into();
        let value = value.into();
        self.base_attributes.push([key, value]);
        self
    }

    pub fn count<N: Into<Cow<'static, str>>>(&mut self, name: N, count: &'static IntCounter) -> RegisterHelper {
        self.metric(name, &count.0, MetricType::IntCounter)
    }

    pub fn gauge<N: Into<Cow<'static, str>>>(&mut self, name: N, gauge: &'static IntGauge) -> RegisterHelper {
        self.metric(name, &gauge.0, MetricType::IntGauge)
    }

    fn metric<N: Into<Cow<'static, str>>>(&mut self, name: N, value: &'static AtomicU64, metric_type: MetricType) -> RegisterHelper {
        let mut helper = self.empty();
        helper.metric(name, value, metric_type);
        helper
    }

    pub fn group<N: Into<Cow<'static, str>>>(&mut self, prefix: N) -> RegisterHelper {
        self.start(Some(prefix))
    }

    pub fn empty(&mut self) -> RegisterHelper {
        self.start::<String>(None)
    }

    fn start<N: Into<Cow<'static, str>>>(&mut self, prefix: Option<N>) -> RegisterHelper {
        let attributes = self.base_attributes.clone();

        let name_prefix = match (&self.name_prefix, prefix) {
            (Some(prefix), None) => Some(Cow::Owned(prefix.clone())),
            (None, Some(prefix)) => Some(prefix.into()),
            (Some(a), Some(b)) => {
                let b = b.into();
                Some(Cow::Owned(format!("{}_{}", a, b)))
            }
            (None, None) => None,
        };

        RegisterHelper {
            metrics: self.metrics,
            name_prefix,
            attributes,
            registered: Vec::new(),
        }
    }
}

pub struct RegisterHelper<'a> {
    name_prefix: Option<Cow<'static, str>>,
    metrics: &'a mut Vec<RegisteredMetric>,
    attributes: Vec<[Cow<'static, str>; 2]>,
    registered: Vec<RegisteredMetric>,
}

impl RegisterHelper<'_> {
    pub fn attr<K: Into<Cow<'static, str>>, V: Into<Cow<'static, str>>>(&mut self, key: K, value: V) -> &mut Self {
        let key = key.into();
        let value = value.into();
        self.attributes.push([key, value]);
        self
    }

    pub fn count<N: Into<Cow<'static, str>>>(&mut self, name: N, count: &'static IntCounter) -> &mut Self {
        self.metric(name, &count.0, MetricType::IntCounter)
    }

    pub fn gauge<N: Into<Cow<'static, str>>>(&mut self, name: N, gauge: &'static IntGauge) -> &mut Self {
        self.metric(name, &gauge.0, MetricType::IntGauge)
    }

    pub fn metric<N: Into<Cow<'static, str>>>(&mut self, name: N, value: &'static AtomicU64, metric_type: MetricType) -> &mut Self {
        let name = match &self.name_prefix {
            Some(prefix) => Cow::Owned(format!("{}_{}", prefix, name.into())),
            None => name.into(),
        };

        self.registered.push(RegisteredMetric {
            metric_type,
            name,
            value,
            attributes: Vec::new(),
        });

        self
    }
}

impl Drop for RegisterHelper<'_> {
    fn drop(&mut self) {
        for mut reg in self.registered.drain(..) {
            reg.attributes = self.attributes.clone();
            self.metrics.push(reg);
        }
        self.metrics.sort_by_key(|item| SortKey {
            name: item.name.clone(),
            metric: item.metric_type,
        });
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct SortKey {
    name: Cow<'static, str>,
    metric: MetricType,
}


#[cfg(test)]
mod test {
    use std::sync::Arc;

    use crate::{IntCounter, IntGauge, PromMetricRegistry};

    #[derive(Debug, Default)]
    struct Met {
        a: IntCounter,
        b: IntCounter,
        c: IntGauge,
    }

    #[test]
    fn metrics_test() {
        let met = Arc::new(Met::default());
        let mut reg = PromMetricRegistry::new();
        reg.base_attributes.push(["prefix".into(), "set".into()]);

        reg.register_fn(&met, |m, reg| {
            reg.name_prefix("base_prefix");

            reg.group("prefix")
                .count("a", &m.a)
                .count("b", &m.b)
                .attr("test", "2");

            reg.gauge("c", &m.c);
        });

        println!("{}", reg);
    }
}

