TARGET := x86_64-pc-windows-gnu
BIN := Course-kudasai
OUT := output

.PHONY: all exe clean

all: exe

exe:
	cargo build --release --target $(TARGET)
	mkdir -p $(OUT)
	cp target/$(TARGET)/release/$(BIN).exe $(OUT)/

clean:
	cargo clean
	rm -rf $(OUT)
