extern crate rustc_serialize;
extern crate time;
extern crate uuid;
use std::io::prelude::*;
use std::net::TcpStream;
use rustc_serialize::{Decodable, Decoder, json};
use influent::create_client;
use influent::client::Client;
use influent::client::Credentials;
use influent::measurement::{Measurement, Value};
use output_args::*;
#[test]
fn test_autodecode() {
    let test_str = "{\"health\":{\"health\":{\"health_services\":[{\"mons\":[{\"name\":\"chris-local-machine-1\",\"kb_total\":232205304,\"kb_used\":81823684,\"kb_avail\":138563228,\"avail_percent\":59,\"last_updated\":\"2015-10-07 12:19:51.281273\",\"store_stats\":{\"bytes_total\":5408347,\"bytes_sst\":0,\"bytes_log\":4166001,\"bytes_misc\":1242346,\"last_updated\":\"0.000000\"},\"health\":\"HEALTH_OK\"},{\"name\":\"chris-local-machine-2\",\"kb_total\":232205304,\"kb_used\":79803236,\"kb_avail\":140583676,\"avail_percent\":60,\"last_updated\":\"2015-10-07 12:19:23.247120\",\"store_stats\":{\"bytes_total\":6844874,\"bytes_sst\":0,\"bytes_log\":5602535,\"bytes_misc\":1242339,\"last_updated\":\"0.000000\"},\"health\":\"HEALTH_OK\"},{\"name\":\"chris-local-machine-3\",\"kb_total\":232205304,\"kb_used\":78650196,\"kb_avail\":141736716,\"avail_percent\":61,\"last_updated\":\"2015-10-07 12:19:07.182466\",\"store_stats\":{\"bytes_total\":6531182,\"bytes_sst\":0,\"bytes_log\":5288894,\"bytes_misc\":1242288,\"last_updated\":\"0.000000\"},\"health\":\"HEALTH_OK\"}]}]},\"summary\":[],\"timechecks\":{\"epoch\":6,\"round\":38,\"round_status\":\"finished\",\"mons\":[{\"name\":\"chris-local-machine-1\",\"skew\":\"0.000000\",\"latency\":\"0.000000\",\"health\":\"HEALTH_OK\"},{\"name\":\"chris-local-machine-2\",\"skew\":\"0.000000\",\"latency\":\"0.000977\",\"health\":\"HEALTH_OK\"},{\"name\":\"chris-local-machine-3\",\"skew\":\"0.000000\",\"latency\":\"0.000818\",\"health\":\"HEALTH_OK\"}]},\"overall_status\":\"HEALTH_OK\",\"detail\":[]},\"fsid\":\"1bb15abc-4158-11e5-b499-00151737cf98\",\"election_epoch\":6,\"quorum\":[0,1,2],\"quorum_names\":[\"chris-local-machine-1\",\"chris-local-machine-2\",\"chris-local-machine-3\"],\"monmap\":{\"epoch\":2,\"fsid\":\"1bb15abc-4158-11e5-b499-00151737cf98\",\"modified\":\"2015-10-07 10:45:23.255204\",\"created\":\"0.000000\",\"mons\":[{\"rank\":0,\"name\":\"chris-local-machine-1\",\"addr\":\"10.0.2.22:6789/0\"},{\"rank\":1,\"name\":\"chris-local-machine-2\",\"addr\":\"10.0.2.78:6789/0\"},{\"rank\":2,\"name\":\"chris-local-machine-3\",\"addr\":\"10.0.2.141:6789/0\"}]},\"osdmap\":{\"osdmap\":{\"epoch\":9,\"num_osds\":3,\"num_up_osds\":3,\"num_in_osds\":3,\"full\":false,\"nearfull\":false}},\"pgmap\":{\"pgs_by_state\":[{\"state_name\":\"active+clean\",\"count\":192}],\"version\":487,\"num_pgs\":192,\"data_bytes\":4970896648,\"bytes_used\":252251439104,\"bytes_avail\":424777154560,\"bytes_total\":713334693888,\"write_bytes_sec\":26793300,\"op_per_sec\":8},\"mdsmap\":{\"epoch\":1,\"up\":0,\"in\":0,\"max\":1,\"by_rank\":[]}}";
    let decoded: CephHealth = json::decode(test_str).unwrap();
    println!("Decoded: {:?}", decoded);
}

#[derive(Debug, RustcDecodable)]
pub struct CephHealth {
    pub election_epoch: u64,
    pub fsid: uuid::Uuid,
    pub health: Health,
    pub quorum: Vec<u64>,
    pub quorum_names: Vec<String>,
    pub pgmap: PgMap,
    pub monmap: MonMap,
    pub osdmap: OsdMap,
}

fn get_time() -> f64 {
    let now = time::now();
    let milliseconds_since_epoch = now.to_timespec().sec * 1000;
    return milliseconds_since_epoch as f64;
}

impl CephHealth{

    pub fn log(&self, args: &Args) {
        CephHealth::log_to_stdout(args, self);
        CephHealth::log_to_influx(args, self);
        CephHealth::log_to_carbon(args, self);
    }

    fn log_to_stdout(args: &Args, ceph_event: &CephHealth) {
        if args.outputs.contains(&"stdout".to_string()) {
            println!("{:?}", ceph_event);
        }
    }

    fn log_to_influx(args: &Args, ceph_event: &CephHealth) {
        if args.outputs.contains(&"influx".to_string()) && args.influx.is_some() {
            let influx = &args.influx.clone().unwrap();
            let credentials = Credentials {
                username: influx.user.as_ref(),
                password: influx.password.as_ref(),
                database: "ceph",
            };
            let host = format!("http://{}:{}", influx.host, influx.port);
            let hosts = vec![host.as_ref()];
            let client = create_client(credentials, hosts);

            let mut measurement = Measurement::new("monitor");
            measurement.add_field("ops",
                                  Value::Integer(ceph_event.pgmap.op_per_sec.unwrap_or(0) as i64));
            measurement.add_field("writes",
                                  Value::Integer(ceph_event.pgmap.write_bytes_sec.unwrap_or(0) as i64));
            measurement.add_field("reads",
                                  Value::Integer(ceph_event.pgmap.read_bytes_sec.unwrap_or(0) as i64));
            measurement.add_field("data", Value::Integer(ceph_event.pgmap.data_bytes as i64));
            measurement.add_field("used", Value::Integer(ceph_event.pgmap.bytes_used as i64));
            measurement.add_field("avail", Value::Integer(ceph_event.pgmap.bytes_avail as i64));
            measurement.add_field("total", Value::Integer(ceph_event.pgmap.bytes_total as i64));
            measurement.add_field("osds",
                                  Value::Integer(ceph_event.osdmap.osdmap.num_osds as i64));
            let res = client.write_one(measurement, None);

            debug!("{:?}", res);
        }
    }

    fn log_packet_to_carbon(carbon_url: &str, carbon_data: String) -> Result<(), String> {
        let mut stream = try!(TcpStream::connect(carbon_url).map_err(|e| e.to_string()));
        let bytes_written = try!(stream.write(&carbon_data.into_bytes()[..])
                                       .map_err(|e| e.to_string()));
        info!("Wrote: {} bytes to graphite", &bytes_written);
        Ok(())
    }

    fn log_to_carbon(args: &Args, ceph_event: &CephHealth) {
        if args.outputs.contains(&"carbon".to_string()) && args.carbon.is_some() {
            let carbon = &args.carbon.clone().unwrap();
            let carbon_data = ceph_event.to_carbon_string(&carbon.root_key);

            let carbon_host = &carbon.host;
            let carbon_port = &carbon.port;
            let carbon_url = format!("{}:{}", carbon_host, carbon_port);
            // println!("{}", carbon_data)
            let _ = CephHealth::log_packet_to_carbon(carbon_url.as_ref(), carbon_data);
        }
    }
    pub fn decode(json_data: &str) -> Result<Self, json::DecoderError> {
        let decode: CephHealth = try!(json::decode(json_data));
        return Ok(decode);
    }

    pub fn to_carbon_string(&self, root_key: &String) -> String {
        let ops_per_sec = match self.pgmap.op_per_sec {
            Some(ops) => ops,
            None => 0,
        };
        let write_bytes_sec = match self.pgmap.write_bytes_sec {
            Some(write_bytes_sec) => write_bytes_sec,
            None => 0,
        };
        let read_bytes_sec = match self.pgmap.read_bytes_sec {
            Some(read_bytes_sec) => read_bytes_sec,
            None => 0,
        };
        format!(r#"{root_key}.{} {} {timestamp}
{root_key}.{} {} {timestamp}
{root_key}.{} {} {timestamp}
{root_key}.{} {} {timestamp}
{root_key}.{} {} {timestamp}
{root_key}.{} {} {timestamp}
{root_key}.{} {} {timestamp}
{root_key}.{} {} {timestamp}
"#,
                "osds",
                self.osdmap.osdmap.num_osds,
                "ops",
                ops_per_sec,
                "write_bytes",
                write_bytes_sec,
                "read_bytes",
                read_bytes_sec,
                "data",
                self.pgmap.data_bytes,
                "used",
                self.pgmap.bytes_used,
                "avail",
                self.pgmap.bytes_avail,
                "total",
                self.pgmap.bytes_total,
                root_key = root_key.clone(),
                timestamp = get_time() / 1000.0)
    }
}

#[derive(Debug, RustcDecodable)]
pub struct Health {
    pub detail: Vec<String>,
    pub health: SubHealth,
    pub overall_status: String,
    pub summary: Vec<SummaryDetail>,
    pub timechecks: TimeCheck,
}

#[derive(Debug, RustcDecodable)]
pub struct SummaryDetail {
    pub severity: String,
    pub summary: String,
}

#[derive(Debug, RustcDecodable)]
pub struct SubHealth {
    pub health_services: Vec<HealthService>,
}

#[derive(Debug, RustcDecodable)]
pub struct HealthService {
    pub mons: Vec<MonHealthDetails>,
}

#[derive(Debug, RustcDecodable)]
pub struct TimeCheck {
    pub epoch: u64,
    pub mons: Vec<MonHealth>,
    pub round: u64,
    pub round_status: String,
}

#[derive(Debug, RustcDecodable)]
pub struct MonHealthDetails {
    pub avail_percent: u64,
    pub health: String,
    pub kb_avail: u64,
    pub kb_total: u64,
    pub kb_used: u64,
    pub last_updated: String,
    pub name: String,
    pub store_stats: MonStoreStat,
}

#[derive(Debug, RustcDecodable)]
pub struct MonStoreStat {
    pub bytes_log: u64,
    pub bytes_misc: u64,
    pub bytes_sst: u64,
    pub bytes_total: u64,
    pub last_updated: String,
}

#[derive(Debug, RustcDecodable)]
pub struct MonHealth {
    pub health: String,
    pub latency: String,
    pub name: String,
    pub skew: String,
}

pub struct MdsMap {
    pub epoch: u64,
    pub by_rank: Vec<String>,
    pub in_map: u64,
    pub max: u64,
    pub up: u64,
}

impl Decodable for MdsMap{
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, D::Error> {
        decoder.read_struct("root", 0, |decoder| {
          decoder.read_struct_field("mdsmap", 0, |decoder| {
             Ok(MdsMap{
              epoch: try!(decoder.read_struct_field("epoch", 0, |decoder| Decodable::decode(decoder))),
              up: try!(decoder.read_struct_field("up", 0, |decoder| Decodable::decode(decoder))),
              in_map: try!(decoder.read_struct_field("in", 0, |decoder| Decodable::decode(decoder))),
              max: try!(decoder.read_struct_field("max", 0, |decoder| Decodable::decode(decoder))),
              by_rank: try!(decoder.read_struct_field("by_rank", 0, |decoder| Decodable::decode(decoder))),
            })
          })
        })
    }
}

#[derive(Debug, RustcDecodable)]
pub struct MonMap {
    pub epoch: u64,
    pub fsid: String,
    pub modified: String,
    pub created: String,
    pub mons: Vec<Mon>,
}

#[derive(Debug, RustcDecodable)]
pub struct PgMap {
    pub bytes_avail: u64,
    pub bytes_total: u64,
    pub bytes_used: u64,
    pub read_bytes_sec: Option<u64>,
    pub write_bytes_sec: Option<u64>,
    pub op_per_sec: Option<u64>,
    pub data_bytes: u64,
    pub num_pgs: u64,
    pub pgs_by_state: Vec<PgState>,
    pub version: u64,
}

#[derive(Debug, RustcDecodable)]
pub struct OsdMap {
    pub osdmap: SubOsdMap,
}

#[derive(Debug, RustcDecodable)]
pub struct SubOsdMap {
    pub epoch: u64,
    pub num_osds: u64,
    pub num_up_osds: u64,
    pub num_in_osds: u64,
    pub full: bool,
    pub nearfull: bool,
}

#[derive(Debug, RustcDecodable)]
pub struct PgState {
    pub count: u64,
    pub state_name: String,
}

#[derive(Debug, RustcDecodable)]
pub struct Mon {
    pub rank: u64,
    pub name: String,
    pub addr: String,
}
