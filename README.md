## arc-metrics

#### Why does this exist?
A lot of data structures & algorithms I write greatly benefit from having counters and gauges for
monitoring and testing assumption in production. Sometimes there's multiple instances and things
get harder when libraries register metrics to a global state. This library tries to make it easy
to create a default Metrics structure which application can register for monitoring however they
want.

#### Example Usage (todo: test example)

```rust
#[derive(Default)]
pub struct MapWrapper<K: Hash + Eq, V> {
    map: HashMap<K, V>,
    metrics: Arc<MapWrapperMetrics>,
}

impl<K: Hash + Eq, V> MapWrapper<K, V> {
    pub fn get(&self, key: &K) -> Option<&V> {
        match self.map.get(key) {
            None => {
                self.metrics.missing_count.inc();
                None
            }
            Some(v) => {
                self.metrics.get_count.inc();
                Some(v)
            }
        }
    }

    pub fn insert(&mut self, key: K, value: V) {
        if self.map.insert(key, value).is_some() {
            self.metrics.replace_count.owned_inc();
        } else {
            self.metrics.insert_count.owned_inc();
        }
    }
}

#[derive(Default)]
pub struct MapWrapperMetrics {
    pub insert_count: IntCounter,
    pub replace_count: IntCounter,
    pub get_count: IntCounter,
    pub missing_count: IntCounter,
}

impl RegisterableMetric for MapWrapperMetrics {
    fn register(&'static self, register: &mut RegisterAction) {
        register.count("insert", &self.insert_count)
            .attr("result", "new");
        register.count("insert", &self.replace_count)
            .attr("result", "replace");
        register.count("get", &self.get_count)
            .attr("result", "exists");
        register.count("get", &self.missing_count)
            .attr("result", "missing");
    }
}

```
