//! Handler for `tokenless mcp start`.

use crate::mcp;

/// Handle `tokenless mcp start`.
pub(crate) fn handle() {
    mcp::run_mcp();
}
