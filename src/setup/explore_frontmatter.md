---
name: Explore
description: Fast read-only search agent for locating code. Use it to find files by pattern, grep for symbols or keywords, or answer "where is X defined / which files reference Y." Do NOT use it for code review, design-doc auditing, cross-file consistency checks, or open-ended analysis — it reads excerpts rather than whole files and will miss content past its read window. When calling, specify search breadth: "quick" for a single targeted lookup, "medium" for moderate exploration, or "very thorough" to search across multiple locations and naming conventions.
disallowedTools: Write, Edit, NotebookEdit
---
