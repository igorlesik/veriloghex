# Parse Verilog-format .hex file

It is a common task to load a Verilog-format .hex file containing data
or a program image into the simulator's memory via JTAG, PCI, or similar interfaces.
The `veriloghex::Reader` provides an `Iterator` interface to iterate over the records in a .hex file,
allowing the user to load the data bytes into memory as needed.

`no_std` compatible for use in embedded environments, for example, RISC-V baremetal code.

## Iterating over bytes example

### Print all records

```ignore
static TEXT_STR: &str = r#"
@81000000
09 A0 F3 22 20 34 63 84 02 00 6F 00 E0 57 81 40
01 41 81 41 01 42 81 42 01 43 81 43 01 44 81 44"#;

let reader = crate::Reader::new(TEXT_STR);
for data in reader {
    std::println!("{}", data.unwrap());
}
```

Output:
```ignore
new address: 0x81000000
0x81000000: 09
0x81000001: A0
0x81000002: F3
0x81000003: 22
```

### Send bytes over PCI

```
fn load_program(bar: &dyn PciRegion, prog_contents: &str) -> anyhow::Result<()> {
    let reader = veriloghex::Reader::new(prog_contents);
    for item in reader {
        let rec = item.unwrap();
        if let veriloghex::Record::Data { addr, value } = rec {
            match value {
                veriloghex::DataType::U8(val_u8) => {
                    std::println!("{:#X}: {:02X}", addr, val_u8);
                    bar.write_u8(addr, val_u8)?;
                }
                _ => Err(anyhow::anyhow!("Unsupported data type"))?,
            }
        }
    }
    Ok(())
}
```

## Grouping bytes example

### Print all records

```ignore
static TEXT_STR: &str = r#"
@81000000
09 A0 F3 22 20 34 63 84 02 00 6F 00 E0 57 81 40
01 41 81 41 01 42 81 42 01 43 81 43 01 44 81 44"#;

let reader = crate::Reader::new_with_options(TEXT_STR, crate::ReaderOptions { group: true });
for data in reader {
    std::println!("{}", data.unwrap());
}
```

Output:
```ignore
new address: 0x81000000
0x81000000: 8463342022F3A009
0x81000008: 408157E0006F0002
0x81000010: 4281420141814101
```
