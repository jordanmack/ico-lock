# Simple Token Sale Lock Script

A simple Lock Script for handling the sale of [SUDT](https://talk.nervos.org/t/rfc-simple-udt-draft-spec/4333) tokens for CKBytes on Nervos CKB.

The Lock Script can be added to any SUDT Cell to enable any user to buy SUDT tokens for a predefined price in CKBytes.

> This project is still under active development and should not be used in production environments.

## Example Transaction

Below is an example of a basic purchase transaction. John purchases 10 SUDT tokens from the Token Sale Cell at a cost of 1 CKByte per SUDT token.

Note: The capacity and cost amounts are normally specified in Shannons, but have been simplified to CKBytes for readability.

![Example Token Purchase](resources/Token-Sale-Lock-Basic-Purchase.png)

## Constraints
The Token Sale Lock enforces the following constraints to ensure proper operation.

1. The arguments must be equal or greater than 40 bytes in length.
2. If an input Cell's lock hash matches that specified in the args, owner mode is then enabled and the Cell unlocks unconditionally.
3. There must be exactly one input Cell with the Token Sale Lock Script and exactly one output Cell with the Token Sale Lock Script.
4. The Type Script of both the input Token Sale Cell and output Token Sale Cell must match.
5. The cost of SUDTs in Shannons must be greater than or equal to 1.
6. The capacity on the output Token Sale Cell must be equal or higher than on the input Token Sale Cell.
7. The SUDT amount of the output Token Sale Cell must be equal or lower than the input Token Sale Cell.
8. The capacity difference between the input/output Token Sale Cells divided by the cost must equal the SUDT amount difference between the input/output Token Sale Cells.

## Building

This project is built in Rust using the [Capsule](https://github.com/nervosnetwork/capsule) development framework.

### Supported Environments
- Linux
- MacOS
- Windows (WSL2)

### Prerequisites
- [Capsule](https://github.com/nervosnetwork/capsule)

### Building a debug binary:

``` sh
capsule build
```

### Running tests on the debug binary:

``` sh
capsule test
```

### Building a release binary:

``` sh
capsule build --release
```

### Running tests on the release binary:

``` sh
capsule test --release
```

## Usage

### Args Definition
- Owner lock script hash (32 bytes)
- Cost per token in CKByte Shannons. (u64 LE 8 bytes)
- A unique identifier. (u32 LE 4 bytes)

> Note: The unique identifier is optional, but highly recommended because it allows for multiple Token Sale Cells to exist in the same transaction and provides an easy way for third party analytics to track an individual Cell. Using a u32 is the recommended guideline, but any form of unique identifier will work and can safely exceed 4 bytes.

The total size of the args should be a minimum of 44 bytes, or 40 bytes if no identifier is specified.

> Warning: Failure to supply proper arguments to the Lock Script can result in the permanent loss of SUDT tokens.

### Owner Mode

Administrative control of the Token Sale Lock is enabled using the Owner Input Recognition design pattern. If any input Cell in a transaction has a Lock Script Hash that matches the first 32 bytes of the args provided to the Token Sale Lock, then owner mode is enabled.

Owner mode allows the following actions:
- Add or remove CKBytes from the Cell.
- Add or remove SUDT tokens from the Cell.
- Update the token cost.
- Removal of the Token Sale Lock in favor of a different lock.

## License
[MIT](LICENSE)