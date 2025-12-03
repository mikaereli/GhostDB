# GhostDB

**GhostDB** is a high-performance, streaming CLI tool designed to anonymize large SQL database dumps (PostgreSQL format). It processes data efficiently without loading the entire file into memory, making it suitable for gigabyte-sized dumps.

## Features

*   **High Performance:** Streaming I/O using buffered reading/writing. Low memory footprint.
*   **Smart Mode:** Automatically detects sensitive columns (PII) and suggests an anonymization plan.
*   **Deterministic:** Uses a seed-based RNG. "Alice" will always become "Bob" (or "Alexander") across different runs, preserving data consistency.
*   **Flexible Strategies:** Supports full replacement (Faker), partial masking (`a***@example.com`), fixed values, and preserving critical data (IDs, timestamps, prices).
*   **Interactive Wizard:** Fine-tune your configuration via a CLI menu without manually editing YAML files.

## Installation

Clone the repository and build using Cargo:

```bash
git clone https://github.com/your-repo/ghostdb.git
cd ghostdb
cargo build --release
```

The binary will be available at `./target/release/ghostdb`.

## Usage

### 1. The Magic "Smart Run" (Recommended)

Simply point GhostDB to your dump file. It will scan the schema, propose a plan, and ask for confirmation.

```bash
./ghostdb --input dump.sql
```

*   **Scans** the file for tables and columns.
*   **Identifies** PII (Email, Phone, Name) and business data (Prices, Dates, IDs).
*   **Proposes** a safe configuration.
*   **Interactive:** You can choose to "Run" immediately or "Customize" specific columns via a menu.

### 2. Generate Configuration (`scan`)

If you want to generate a YAML configuration file for later use (CI/CD pipelines, etc.):

```bash
# Standard scan (print to stdout)
./ghostdb scan --input dump.sql > config.yaml

# Interactive scan (wizard mode)
./ghostdb scan --interactive --input dump.sql
```

### 3. Headless Execution (`run`)

Run with a pre-defined configuration file (ideal for automated scripts):

```bash
./ghostdb run --input dump.sql --output anonymized.sql --config config.yaml
```

## Configuration Strategies

GhostDB supports the following strategies for columns:

| Strategy | Description | Example |
| :--- | :--- | :--- |
| `keep` | Preserves the original value. (Default for IDs, Dates, Prices) | `123` -> `123` |
| `email` | Replaces with a deterministic fake email. | `alice@work.com` -> `bob@example.org` |
| `phone` | Replaces with a fake phone number. | `+1-555-0199` -> `202-555-0142` |
| `first_name` | Replaces with a random first name. | `Alice` -> `Sarah` |
| `last_name` | Replaces with a random last name. | `Smith` -> `Connor` |
| `full_name` | Replaces with a full name. | `Alice Smith` -> `Sarah Connor` |
| `mask` | Partially masks the value. | `alice@work.com` -> `a***@w***.com` |
| `fixed` | Replaces with a static string. | `123 Main St` -> `REDACTED ADDRESS` |

### Example `config.yaml`

```yaml
tables:
  public.users:
    columns:
      id: keep
      email: email
      password_hash: !fixed "REDACTED_HASH"
      full_name: mask
      created_at: keep
  public.orders:
    columns:
      total_amount: keep
      shipping_address: !fixed "ANONYMIZED"
```

## Privacy & Determinism

GhostDB uses a seeded random number generator (`rand` + `sha256` hash of the original value + global seed).

*   **Same Seed + Same Input = Same Output.**
*   This ensures that foreign key relationships (e.g., if you anonymize user emails that are used as keys) *might* be preserved if they are strings, but typically you should **Keep** IDs (`id`, `user_id`) to maintain referential integrity.

## License

MIT

