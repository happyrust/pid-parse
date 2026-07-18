# Define full parsing as evidence-complete read-only

The project defines "full PID parsing" as evidence-complete read parsing: every source byte must be decoded, explicitly classified for audit or investigation, or preserved as unknown with provenance. We deliberately do not require guessed semantics or semantic write-back, because forcing zero unknown bytes would turn unsupported inferences into a misleading stable contract.
