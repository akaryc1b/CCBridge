# 手动同步到 Codeup

GitHub 是主仓库和构建发布源，Codeup 是第二远端。按需要手动推送，不配置自动同步，也不需要在 GitHub 保存 Codeup 凭据。

在本机已配置 Codeup Git 认证的情况下执行：

```sh
git push --atomic codeup --all
git push --atomic codeup --tags
```

以上命令推送本地全部分支和标签，不强推、不删除远端引用。同步前先确认本地分支包含要发布的 GitHub 提交；远端存在独立提交或同名标签冲突时，应核实差异，不强制覆盖。

Codeup 远端地址：`https://codeup.aliyun.com/62a4de12a61bf880d4caa81e/CCBridge.git`。

现有 GitHub 构建与发布流程保持不变，应用版本不受此调整影响。
