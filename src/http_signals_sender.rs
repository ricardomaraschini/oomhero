use super::arguments;
use super::errors;
use super::processes;
use super::signals;
use serde::Deserialize;
use serde::Serialize;

// HttpSignalSender issues a htp request instead of sending an unix signal to a process. The http
// request is read from a file on disk and should comply with the HttpNotificationConfig format.
// The method is POST and the content is defined by the HttpNotificationBody struct.
pub struct HttpSignalSender {
    config: HttpNotificationConfig,
}

// HttpNotificationConfig represents what the user can configure when sending notifications. Not
// much can be configured other than the url and a few headers. The method is hardcoded to post
// as we send data out.
#[derive(Deserialize, Serialize)]
pub struct HttpNotificationConfig {
    pub url: String,
    pub headers: Vec<Header>,
}

// Header is a http  header (name and respective value).
#[derive(Deserialize, Serialize)]
pub struct Header {
    pub name: String,
    pub value: String,
}

// HttpNotificationBody is what we send out during each notification. Contain the information
// about the severity of the alert, the process and the last collected data about it.
#[derive(Serialize)]
struct HttpNotificationBody {
    severity: arguments::CheckerResult,
    process: processes::Process,
    collected_data: processes::CollectedData,
}

impl HttpSignalSender {
    // new returns a new HttpSignalSender or an error if we can't parse the config file (yaml)
    // into a HttpNotificationConfig.
    pub fn new(path: String) -> Result<Self, errors::Error> {
        let content = std::fs::read_to_string(path)?;
        let config: HttpNotificationConfig = serde_yaml::from_str(&content)?;
        Ok(Self { config })
    }
}

impl signals::Sender for HttpSignalSender {
    // Send issues the http request sending.
    fn send(
        &self,
        severity: &arguments::CheckerResult,
        process: &processes::Process,
        cd: &processes::CollectedData,
    ) -> Result<(), errors::Error> {
        let body = HttpNotificationBody {
            severity: severity.clone(),
            process: process.clone(),
            collected_data: cd.clone(),
        };

        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(3)))
            .build()
            .into();

        let mut builder = agent.post(&self.config.url);
        for header in &self.config.headers {
            builder = builder.header(&header.name, &header.value);
        }
        builder.send_json(body)?;
        Ok(())
    }
}
