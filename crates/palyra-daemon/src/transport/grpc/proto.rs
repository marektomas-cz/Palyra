pub mod palyra {
    pub mod common {
        pub mod v1 {
            tonic::include_proto!("palyra.common.v1");
        }
    }

    pub mod gateway {
        pub mod v1 {
            tonic::include_proto!("palyra.gateway.v1");
        }
    }

    pub mod cron {
        pub mod v1 {
            tonic::include_proto!("palyra.cron.v1");
        }
    }

    pub mod memory {
        pub mod v1 {
            tonic::include_proto!("palyra.memory.v1");
        }
    }

    pub mod auth {
        pub mod v1 {
            tonic::include_proto!("palyra.auth.v1");
        }
    }

    pub mod node {
        pub mod v1 {
            tonic::include_proto!("palyra.node.v1");
        }
    }

    pub mod browser {
        pub mod v1 {
            tonic::include_proto!("palyra.browser.v1");
        }
    }
}
