"""`python -m tellegen.mcp` — serve the tool surface over stdio."""

# Resolved through the package's lazy `__getattr__`, so a missing `mcp` extra
# reports what to install instead of a bare ModuleNotFoundError.
import tellegen.mcp

tellegen.mcp.main()
