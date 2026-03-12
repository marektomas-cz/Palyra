use clap::ValueEnum;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum InitModeArg {
    Local,
    Remote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum InitTlsScaffoldArg {
    None,
    BringYourOwn,
    SelfSigned,
}
