# block-buffer 0.10.4 公告适用性核实

复核日期：2026-10-27。仅适用于锁定的 **block-buffer 0.10.4**、Mac ARM/Intel App 构建图；2026-11-26 到期。其他版本、目标及其他公告仍阻止发布。

## 结论与依据

[GHSA-qwgh-2vcv-g2f7](https://github.com/advisories/GHSA-qwgh-2vcv-g2f7) 的版本范围覆盖了0.10.4，但描述的机制是：压缩回调panic后，缓冲区内嵌的游标字节被输入覆盖，catch_unwind后继续访问造成越界。

[0.10.4的原始源码](https://github.com/RustCrypto/utils/blob/block-buffer-v0.10.4/block-buffer/src/lib.rs) 中，`BlockBuffer` 的 `buffer` 与 `pos: u8` 是独立字段；`digest_blocks` 复制输入只写 `buffer`，回调不接收 `pos`。panic时保留旧的合法游标，而不是把输入字节当游标。该版本也没有公告示例中的 `ReadBuffer` 类型。

因此，本次按**指定机制不适用于此精确版本**记录有限期复核，不宣称上游公告已撤销，也不关闭安全检查。

当前路径：`block-buffer 0.10.4 → digest 0.10.7 → sha2 0.10.9 → tauri-codegen 2.7.3 → tauri-macros → tauri`。它用于构建时资产/CSP生成；Linux Runner图不含此版本。升级锁文件或改变调用路径需重新复核。

## 最小复现核对

在独立临时Cargo项目依赖 `block-buffer = "=0.10.4"`，执行：

```rust
use block_buffer::{EagerBuffer, generic_array::typenum::U4};
use std::panic::{catch_unwind, AssertUnwindSafe};
let mut buf = EagerBuffer::<U4>::new(&[1, 2]);
assert!(catch_unwind(AssertUnwindSafe(||
    buf.digest_blocks(&[3, 0xff], |_| panic!("fixture panic"))
)).is_err());
assert_eq!(buf.get_pos(), 2);
assert_eq!(buf.get_data().len(), 2);
buf.digest_blocks(&[7, 8], |_| {});
assert_eq!(buf.get_pos(), 0);
```

实测通过：回调panic后游标仍为2，继续读取和处理后为0。普通运行核对游标行为；未执行Miri，不将其写成完整内存安全证明。源代码中游标与输入分离是本次适用性判断的主要依据。
