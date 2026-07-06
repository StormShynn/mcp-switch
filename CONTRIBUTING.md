# Contributing to MCP Switch

Cảm ơn bạn đã quan tâm đến việc đóng góp cho **MCP Switch**! 🎉

Project này sử dụng **Semantic Release** để tự động bump version, tạo tag, và publish release lên GitHub dựa trên commit message của bạn.

## 📝 Conventional Commits

Tất cả commit message **phải** tuân theo chuẩn [Conventional Commits](https://www.conventionalcommits.org/).

### Cấu trúc

```
<type>(<scope>): <mô tả>

<thân message (optional)>

<footer (optional)>
```

Ví dụ:

```
fix(claude): sửa lỗi đọc config khi path có khoảng trắng
feat(ui): thêm bộ lọc theo trạng thái server
docs: cập nhật README
```

### Các type quyết định version bump

| Type | Version bump | Ý nghĩa |
|---|---|---|
| **`fix`** | `patch` (0.1.0 → 0.1.1) | Sửa lỗi |
| **`feat`** | `minor` (0.1.0 → 0.2.0) | Thêm tính năng mới |
| **`BREAKING CHANGE`** (footer) | `major` (0.1.0 → 1.0.0) | Thay đổi không tương thích ngược |
| `chore` | Không bump | Cập nhật dependencies, config |
| `docs` | Không bump | Sửa tài liệu |
| `refactor` | Không bump | Tái cấu trúc code, không thay đổi behavior |
| `style` | Không bump | Format code, lint |
| `test` | Không bump | Thêm/sửa test |
| `perf` | Không bump | Tối ưu hiệu năng |

### Ví dụ chi tiết

#### fix — bump patch

```bash
git commit -m "fix: sửa lỗi crash khi import config không hợp lệ"
```

#### feat — bump minor

```bash
git commit -m "feat: thêm nút refresh servers"
```

#### BREAKING CHANGE — bump major

```bash
git commit -m "feat: chuyển sang format store mới

BREAKING CHANGE: không còn hỗ trợ store version 1"
```

#### Không bump version

```bash
git commit -m "docs: cập nhật hướng dẫn cài đặt"
git commit -m "chore: upgrade tauri lên 2.0"
git commit -m "refactor: tách adapter thành file riêng"
git commit -m "test: thêm unit test cho store"
```

### Scope (phạm vi) — khuyên dùng

Thêm scope để dễ tra cứu:

```bash
git commit -m "fix(adapter): sửa lỗi path trên Windows"
git commit -m "feat(ui): thêm dark mode"
git commit -m "fix(store): lỗi ghi file khi thiếu quyền"
git commit -m "feat(adapter): hỗ trợ OpenCode"
```

### Lưu ý

- Sau dấu `:` phải có **khoảng trắng**: `feat: thêm X` ✅, `feat:thêm X` ❌
- `BREAKING CHANGE` viết ở **footer**, cách dòng trống với thân message
- Type viết **chữ thường**
- Mô tả ngắn gọn, tối đa ~72 ký tự

---

## 🔄 Flow làm việc

```bash
# 1. Code tính năng mới
git commit -m "feat(ui): thêm bảng điều khiển"
git push origin main
# → Semantic Release tự động:
#   - Phân tích commit
#   - Bump version (nếu cần)
#   - Tạo tag v* + GitHub Release
#   - Build file cài đặt cho Windows/macOS/Linux

# 2. Sửa lỗi
git commit -m "fix: crash khi path có dấu cách"
git push origin main
# → Tương tự, bump patch version
```

**Lưu ý**: Nếu commit không có `fix:`, `feat:`, hoặc `BREAKING CHANGE`, sẽ **không có release** mới được tạo.

---

## 🛠️ Cài đặt môi trường phát triển

### Yêu cầu

- [Node.js](https://nodejs.org/) >= 20
- [Rust](https://www.rust-lang.org/) (dùng `rustup`)
- [Tauri CLI](https://v2.tauri.app/start/prerequisites/)

### Quick start

```bash
# Clone
git clone https://github.com/StormShynn/mcp-switch.git
cd mcp-switch

# Cài dependencies
npm install

# Chạy dev mode
npm run tauri dev

# Build
npm run tauri build
```

---

## 📦 Cấu trúc project

```
mcp-switch/
├── src/                    # React frontend
│   ├── App.tsx
│   ├── main.tsx
│   └── lib/
├── src-tauri/              # Rust backend
│   └── src/
│       ├── adapter/        # Adapter cho từng MCP client
│       ├── commands.rs     # Tauri commands
│       ├── store.rs        # Config store
│       └── types.rs
├── scripts/                # Build/CI scripts
├── .github/workflows/      # GitHub Actions
├── .releaserc              # Semantic Release config
└── package.json
```

---

## ✅ Checklist trước khi tạo Pull Request

- [ ] Commit message tuân theo **Conventional Commits**
- [ ] Code build thử: `npm run tauri build`
- [ ] TypeScript check: `npm run typecheck`
- [ ] Không để sót `console.log` debug
- [ ] Nếu thêm tính năng UI: kiểm tra responsive

---

## 🤔 Câu hỏi thường gặp

**Q: Lỡ commit sai type thì sao?**
A: Dùng `git commit --amend` để sửa message. Nếu đã push, dùng force push (cẩn thận nếu có người khác đang làm việc).

**Q: Có cần phải ghi scope không?**
A: Không bắt buộc, nhưng scope giúp dễ tra cứu hơn.

**Q: Tôi muốn tạo release manual?**
A: Dùng `git tag v0.x.x && git push origin v0.x.x` — workflow release cũ vẫn chạy.

**Q: Làm sao để biết commit có tạo release không?**
A: Lên GitHub Actions → workflow `semantic-release` chạy sau mỗi push lên main. Nếu có release, log sẽ hiển thị version mới.

---

Cảm ơn bạn đã đóng góp! Nếu có thắc mắc, hãy tạo [Issue](https://github.com/StormShynn/mcp-switch/issues) trên GitHub.
