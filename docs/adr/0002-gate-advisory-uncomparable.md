# Oracle results are Gate, Advisory, or Uncomparable

Every oracle op is assigned one of three verdicts. A **Gate** op must match the C#
oracle after key-sorted normalization; an **Advisory** op reports differences but
does not fail the run; an **Uncomparable** op cannot be run because the C# side is a
stub or missing. The distinction is deliberate: treating an uncomparable op as a
failure would make CI permanently red for work that has not been implemented yet.

**Consequences**: `tools/oracle-diff` owns the op table. Moving an op from
`Uncomparable` to `Gate` is a deliberate change, not a test flake.
