//! Provider catalog, process seams, adapters, and v2 domain mapping.

pub mod adapter;
pub mod adapters;
pub mod catalog;
pub mod http;
pub mod process;
pub mod v2_map;

pub use adapter::{
    adapter_for, run_login, BoxFuture, CollectionContext, HttpClient, HttpError, HttpResponse,
    LoginError, LoginOutcome, ProviderAdapter,
};
pub use adapters::{
    AmpAdapter, ClaudeAdapter, CodexAdapter, GrokAdapter, AMP_ADAPTER, CLAUDE_ADAPTER,
    CLAUDE_USAGE_URL, CODEX_ADAPTER, GROK_ADAPTER,
};
pub use catalog::{
    descriptor, discover, login_process_argv, CatalogError, CollectionAvailability, Discovery,
    ExecutablePath, ExecutionEnvironment, LoginAvailability, PathRoot, ProviderDescriptor,
    RetryPolicy, AMP, CLAUDE, CODEX, GROK, PROVIDERS,
};
pub use process::{
    run_process, ProcessError, ProcessOutput, ProcessRunner, ProcessSpec, TokioProcessRunner,
};
