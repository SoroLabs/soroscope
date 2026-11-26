# XDR transaction-result decoder

`XdrTransactionResultDecoder` converts Soroban RPC's base64 XDR fields into
serializable, human-readable diagnostics.

- `decode_result_meta(result_meta_xdr)` decodes the `resultMetaXdr` returned by
  `getTransaction`.
- `decode_soroban_meta_xdr(meta_xdr)` decodes standalone `SorobanTransactionMeta`.
- `decode_envelope(envelope_xdr)` extracts each `InvokeHostFunction` contract
  invocation from `envelopeXdr`.
- `decode(result_meta_xdr, Some(envelope_xdr))` combines both sources.

The decoded result includes contract events, diagnostic events, return value,
and non-refundable, refundable, and rent resource fees. The transaction source
is preserved as the XDR debug representation so muxed accounts remain lossless.
