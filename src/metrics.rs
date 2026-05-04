use super::errors::Error;
use super::processes;
use metrics::gauge;
use metrics_exporter_prometheus;

#[derive(Default)]
pub struct Server {}

impl Server {
    // start runs the http handler for the metrics endpoint. this listens on port 9000 by default.
    // no customizations are allowed yet.
    pub fn start(&self) -> Result<(), Error> {
        metrics_exporter_prometheus::PrometheusBuilder::new()
            .install()
            .map_err(Into::into)
    }

    // report_usage updates the gauge representing the memory usage for the provided process.
    fn report_usage(&self, p: &processes::Process, cd: &processes::CollectedData) {
        let labels = [("pid", p.pid.to_string()), ("cmdline", p.cmdline.clone())];
        gauge!("memory_usage", &labels).set(cd.memory_usage());
    }

    // report_oom_score updates the gauge representing the oomscore for the provided process.
    fn report_oom_score(&self, p: &processes::Process, cd: &processes::CollectedData) {
        let labels = [("pid", p.pid.to_string()), ("cmdline", p.cmdline.clone())];
        gauge!("oom_score", &labels).set(cd.oom_score as f32);
    }

    // report_pressure updates the gauges representing the pressure for the provided process. We
    // have gauges for memory, io and cpu pressures.
    fn report_pressure(&self, p: &processes::Process, d: &processes::CollectedData) {
        self.report_pressure_data("memory_pressure", p, &d.pressure.memory);
        self.report_pressure_data("io_pressure", p, &d.pressure.memory);
        self.report_pressure_data("cpu_pressure", p, &d.pressure.memory);
    }

    // report_pressure_data updates the gauge representing a specific pressure data, it can be one
    // of memory, cpu or io pressure data.
    fn report_pressure_data(&self, mtr: &str, p: &processes::Process, d: &processes::PressureData) {
        self.report_pressure_averages(mtr, p, "some", &d.some);
        self.report_pressure_averages(mtr, p, "full", &d.full);
    }

    // report_pressure_averages updates the averages for a given severity level (full or some).
    fn report_pressure_averages(
        &self,
        mtr: &str,
        p: &processes::Process,
        sev: &str,
        d: &processes::PressureAverages,
    ) {
        let severity_windows = [
            ("avg10", d.avg10),
            ("avg60", d.avg60),
            ("avg300", d.avg300),
            ("total", d.total),
        ];
        for (window, value) in severity_windows {
            let labels = [
                ("pid", p.pid.to_string()),
                ("cmdline", p.cmdline.clone()),
                ("severity_level", sev.to_string()),
                ("severity_window", window.to_string()),
            ];
            gauge!(mtr.to_string(), &labels).set(value);
        }
    }

    // report_collected_data makes the provided collected data available through prometheus
    // metrics. we export all the collected data.
    pub fn report_collected_data(&self, p: &processes::Process, d: &processes::CollectedData) {
        self.report_usage(&p, d);
        self.report_oom_score(&p, d);
        self.report_pressure(&p, d);
    }
}
