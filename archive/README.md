# 归档

`scaffolds/media-manager-template/` 保存原根目录 `MediaManager/` 中的默认 Tauri + Vanilla TypeScript 脚手架。它只含 greet 示例；本次整理完整保留了 35 个文件，移动前后 SHA-256 全部一致，未合并到正式业务代码，也未删除内容。

正式项目位于仓库根目录，使用 React 和 `src-tauri/` 数据库层。在根目录运行 npm/Cargo 命令。归档目录不属于活动 Cargo workspace，Vite dev watch 也排除 archive；不要把归档模板的 package.json、权限配置或 greet command 当成正式配置。

恢复原布局时可将该模板移回根目录的 `MediaManager/`；不得覆盖正式项目。归档不是新 Phase，也不包含 Event Service。
