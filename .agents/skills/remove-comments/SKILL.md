---
name: remove-comments
description: special-comments.toml이 허용하지 않은 주석과 Python docstring을 special-comments 도구로 지울 때 사용한다. commit 스킬이 커밋할 파일에 대해 호출하며, 저장소 전체 정리에도 쓴다. 저장소 전체 정리는 필수가 아닌 권장 작업이며, 위험이 있으면 실행 전에 경고하거나 다시 질문한다.
---

# 주석 지우기

주석은 `.agents/tools/special-comments` 도구로만 찾고 지운다. 직접 찾거나 지우지 않는다. 도구는 저장소 루트에서 실행한다.

```bash
cargo run --locked --quiet --manifest-path .agents/tools/special-comments/Cargo.toml -- check <대상>
cargo run --locked --quiet --manifest-path .agents/tools/special-comments/Cargo.toml -- remove <대상>
```

- `commit` 스킬이 호출하면 `<대상>`은 `--staged`다. stage된 파일만 처리한다.
- 저장소 전체를 정리할 때는 `<대상>`이 `--all`이다. git이 추적하는 모든 파일을 처리한다.
- `check`는 허용되지 않은 항목이 있으면 종료 코드 1, 설정이나 git 오류면 2를 낸다. 파일은 바꾸지 않는다.

## 저장소 전체를 정리할 때

이 작업은 권장이며 필수가 아니다. 사용자가 요청하거나, 에이전트가 제안하고 사용자가 동의했을 때 실행한다. 실행 전에 다음에 해당하는지 확인하고, 해당하면 사용자에게 경고하거나 다시 질문한다.

- 커밋되지 않은 변경이 있으면 주석 정리와 다른 변경이 한 커밋에 섞인다.
- 외부에서 가져온 코드가 있으면 라이선스 표기가 지워진다.
- 생성된 코드가 있으면 다시 생성할 때 주석이 되살아난다.
- 외부 도구가 읽는 docstring이 있으면 CLI 도움말이나 API 설명이 바뀐다.
- 대상 파일이 많으면 diff가 커진다.

## 절차

1. `check`로 지워질 주석과 docstring의 목록을 본다.
2. 목록 중 꼭 필요한 종류가 있는지 검토한다. 라이선스 표기, 실행에 필요한 shebang, 도구가 읽는 지시문, 외부 도구가 읽는 docstring이 여기에 해당할 수 있다.
3. 같은 일을 코드나 설정으로 할 수 있으면 주석 대신 그 방법을 쓴다. C/C++의 `/* fallthrough */` 대신 `[[fallthrough]]`를 쓰고, Python의 `# type: ignore` 대신 타입을 고치거나 `typing.cast`를 쓴다.
4. 그래도 필요한 종류는 패턴, 필요한 이유, 해당 주석을 사용자에게 제시하고 승인받는다. 승인된 패턴만 `special-comments.toml`에 추가한다. 거절된 항목을 지워서 깨지는 것이 있으면 알린다.
5. `remove`로 나머지를 지운다.
6. 도구가 건너뛴 파일이 있으면 파일과 이유를 보고하고 어떻게 할지 질문한다. 그 파일의 주석을 직접 지우지 않는다.
7. 지운 주석과 docstring의 수, `special-comments.toml`에 추가한 패턴을 보고한다.
8. 저장소 전체를 정리했다면 커밋은 `commit` 스킬로 한다.
