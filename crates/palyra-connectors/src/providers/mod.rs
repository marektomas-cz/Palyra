use std::sync::Arc;

use crate::core::ConnectorAdapter;

pub mod discord;

#[must_use]
pub fn default_provider_adapters() -> Vec<Arc<dyn ConnectorAdapter>> {
    vec![Arc::new(discord::DiscordConnectorAdapter::default())]
}

#[cfg(test)]
mod tests {
    use crate::protocol::{ConnectorAvailability, ConnectorKind};

    use super::default_provider_adapters;

    #[test]
    fn provider_registry_contains_supported_provider_adapters() {
        let adapters = default_provider_adapters();
        let runtime_surface = adapters
            .iter()
            .map(|adapter| (adapter.kind(), adapter.availability()))
            .collect::<Vec<_>>();

        assert_eq!(
            runtime_surface,
            vec![(ConnectorKind::Discord, ConnectorAvailability::Supported)]
        );
    }
}
