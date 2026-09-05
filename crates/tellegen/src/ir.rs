//! PowerIO IR text for a module: the one serialization Tellegen saves and
//! reads. Every function calls the PowerIO facade; Tellegen holds no reader or
//! writer of its own.

use powerio::{BalancedNetwork, Destination, EmittedOutput, PioModule, PioValue, Source};

/// The artifact name PowerIO gives an IR document written to memory.
const IR_FILE_NAME: &str = "module.pio.json";

/// The PowerIO IR document of `module` as JSON text.
pub fn serialize_module<T>(module: &PioModule<T>) -> Result<String, String>
where
    T: Clone + Into<PioValue>,
{
    let destination = Destination::memory(IR_FILE_NAME).map_err(|e| e.to_string())?;
    let result = powerio::serialize(module, destination).map_err(|e| e.to_string())?;
    let EmittedOutput::Memory { mut artifacts } = result.into_output() else {
        return Err("PowerIO returned a path output for a memory destination".to_owned());
    };
    let Some(artifact) = artifacts.pop() else {
        return Err("PowerIO returned no IR document".to_owned());
    };
    if !artifacts.is_empty() {
        return Err(format!(
            "PowerIO returned {} artifacts for one IR document",
            artifacts.len() + 1
        ));
    }
    String::from_utf8(artifact.into_bytes()).map_err(|e| e.to_string())
}

/// The module a PowerIO IR document describes.
pub fn deserialize_module(text: &str) -> Result<PioModule<PioValue>, String> {
    let source =
        Source::from_memory(IR_FILE_NAME, text.as_bytes().to_vec()).map_err(|e| e.to_string())?;
    powerio::deserialize(source).map_err(|e| e.to_string())
}

/// Narrow a module to its balanced network, keeping every module record.
pub fn balanced_module(module: PioModule<PioValue>) -> Result<PioModule<BalancedNetwork>, String> {
    module.try_map_value(|value| match value {
        PioValue::BalancedNetwork(network) => Ok(network),
        other => Err(format!(
            "PowerIO module holds {}, not a balanced network",
            other.type_name()
        )),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_module_reads_back_from_its_ir_text() {
        let network = crate::model::parse_matpower(crate::model::CASE3).expect("parse");
        let module = PioModule::new(PioValue::BalancedNetwork(network.clone()));
        let text = serialize_module(&module).expect("serialize");
        let back = deserialize_module(&text).expect("deserialize");
        let back = balanced_module(back).expect("balanced");
        assert_eq!(back.value().buses().len(), network.buses().len());
        assert_eq!(back.value().branches().len(), network.branches().len());
    }

    #[test]
    fn a_solution_module_is_not_a_balanced_network() {
        let network = crate::model::parse_matpower(crate::model::CASE3).expect("parse");
        let instance = powerio::DcOpfInstance::from_network(network).expect("instance");
        let module = PioModule::new(PioValue::DcOpfInstance(instance));
        let error = balanced_module(module).expect_err("not a network");
        assert!(error.contains("powerio.DcOpfInstance"), "{error}");
    }
}
