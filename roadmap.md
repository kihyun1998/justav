# justav — Pure Rust Multimedia Toolkit Roadmap

## Design Principles

- **Clean-room 구현**: 기존 C 코드를 번역하지 않는다. 공개 스펙/RFC/ISO 문서 기반으로 새로 설계한다.
- **라이선스**: MIT OR Apache-2.0 듀얼 라이선스. GPL/LGPL 의존성 없음.
- **Safety first**: `unsafe` 최소화. public API에서 panic 금지 (`Result<T, Error>` 강제).
- **`Send + Sync` 기본**: 모든 Context 타입은 `Send`. 내부 가변 상태는 `&mut self`로 보호.
- **Feature-gated**: 코덱/포맷/필터는 각각 feature flag. `default` = 핵심 코덱 + 핵심 포맷.
- **`no_std` 지원 범위**: av-util 코어 (error, rational, pixfmt, samplefmt)는 `no_std` 호환. I/O 의존 크레이트는 `std` 필수.
- **WASM 타겟**: `wasm32-unknown-unknown` 지원 고려. 네트워크/디바이스/HW 가속은 WASM에서 제외.

---

## Project Structure

```
justav/
├── Cargo.toml                  # workspace root
├── crates/
│   ├── av-util/                # 모든 크레이트의 기반 유틸리티
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── error.rs        # Error enum, Result<T> alias
│   │   │   ├── rational.rs     # Rational (시간/비율 연산)
│   │   │   ├── mathematics.rs  # rescale, gcd, timestamp 비교
│   │   │   ├── mem.rs          # aligned alloc, buffer pool
│   │   │   ├── buffer.rs       # refcounted buffer (Arc 기반)
│   │   │   ├── frame.rs        # Frame (audio/video 공용)
│   │   │   ├── side_data.rs    # FrameSideData 타입 + 관리
│   │   │   ├── dict.rs         # key-value 메타데이터
│   │   │   ├── opt.rs          # 옵션 시스템 (key=value 파싱, 타입 검증)
│   │   │   ├── log.rs          # 로깅 (tracing 연동)
│   │   │   ├── imgutils.rs     # 이미지 유틸 (linesize, copy, fill)
│   │   │   ├── samplefmt.rs    # 오디오 샘플 포맷
│   │   │   ├── pixfmt.rs       # 픽셀 포맷 enum + descriptor
│   │   │   ├── channel_layout.rs
│   │   │   ├── timestamp.rs    # PTS/DTS 관리, 불연속(discontinuity) 감지
│   │   │   ├── hash.rs         # md5, sha, crc
│   │   │   ├── base64.rs
│   │   │   ├── fifo.rs         # generic ring buffer
│   │   │   ├── audio_fifo.rs
│   │   │   └── samples.rs      # 오디오 샘플 alloc/copy/silence
│   │   └── tests/
│   │       ├── rational_test.rs
│   │       ├── buffer_test.rs
│   │       ├── frame_test.rs
│   │       └── ...
│   │
│   ├── av-codec/               # 인코딩/디코딩 프레임워크
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── codec.rs        # Codec trait, CodecId enum, registry
│   │   │   ├── context.rs      # CodecContext (encode/decode 상태)
│   │   │   ├── packet.rs       # Packet, side data
│   │   │   ├── codec_par.rs    # CodecParameters
│   │   │   ├── parser.rs       # stream parser
│   │   │   ├── bsf.rs          # bitstream filter trait + chain
│   │   │   ├── decode.rs       # send_packet / receive_frame
│   │   │   ├── encode.rs       # send_frame / receive_packet
│   │   │   ├── error_resilience.rs  # 에러 복구, 프레임 은닉(concealment)
│   │   │   ├── subtitle.rs     # 자막 디코딩/인코딩 (text + bitmap)
│   │   │   └── codecs/         # 코덱 구현체 (feature-gated)
│   │   │       ├── mod.rs
│   │   │       ├── h264/
│   │   │       ├── hevc/
│   │   │       ├── aac/
│   │   │       ├── opus/
│   │   │       ├── vp9/
│   │   │       ├── av1/
│   │   │       ├── flac/
│   │   │       ├── pcm/
│   │   │       ├── mp3/
│   │   │       ├── vorbis/
│   │   │       ├── png/
│   │   │       └── subtitle/   # SRT, ASS, WebVTT 파서
│   │   └── tests/
│   │
│   ├── av-format/              # 컨테이너 먹싱/디먹싱
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── context.rs      # FormatContext (input/output)
│   │   │   ├── stream.rs       # Stream, StreamGroup, Program
│   │   │   ├── io.rs           # IOContext (Read/Write/Seek trait 기반)
│   │   │   ├── io_async.rs     # AsyncIOContext (tokio AsyncRead/Write/Seek)
│   │   │   ├── probe.rs        # 포맷 자동 탐지 (magic bytes)
│   │   │   ├── demux.rs        # Demuxer trait + read_frame
│   │   │   ├── mux.rs          # Muxer trait + write_frame/interleave
│   │   │   ├── seek.rs         # 시크 로직
│   │   │   ├── interleave.rs   # A/V 인터리빙 전략
│   │   │   ├── timestamp.rs    # DTS 정렬, 불연속 처리, 타임베이스 변환
│   │   │   ├── metadata.rs     # 챕터, 첨부파일(앨범아트), 구조화 메타데이터
│   │   │   ├── protocol/       # 네트워크 프로토콜 레이어
│   │   │   │   ├── mod.rs      # Protocol trait
│   │   │   │   ├── file.rs     # 로컬 파일
│   │   │   │   ├── http.rs     # HTTP/HTTPS
│   │   │   │   ├── tcp.rs      # TCP
│   │   │   │   ├── udp.rs      # UDP
│   │   │   │   └── pipe.rs     # stdin/stdout 파이프
│   │   │   └── formats/        # 컨테이너 구현체 (feature-gated)
│   │   │       ├── mod.rs
│   │   │       ├── mp4/        # ISO BMFF + fragmented MP4
│   │   │       ├── matroska/   # MKV/WebM
│   │   │       ├── mpegts/     # MPEG-TS
│   │   │       ├── flv/        # FLV
│   │   │       ├── wav/        # RIFF/WAV
│   │   │       ├── ogg/        # OGG
│   │   │       ├── avi/        # AVI
│   │   │       ├── hls/        # HLS (M3U8 + TS segments)
│   │   │       └── dash/       # DASH (MPD + fMP4 segments)
│   │   └── tests/
│   │
│   ├── av-filter/              # 필터 그래프 프레임워크
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── graph.rs        # FilterGraph 빌드/실행
│   │   │   ├── filter.rs       # Filter trait, pad, link
│   │   │   ├── buffersrc.rs    # 입력 소스
│   │   │   ├── buffersink.rs   # 출력 싱크
│   │   │   ├── formats.rs      # 포맷 네고시에이션
│   │   │   └── filters/        # 필터 구현체 (feature-gated)
│   │   │       ├── mod.rs
│   │   │       ├── video/      # scale, crop, pad, overlay, fps, ...
│   │   │       ├── audio/      # volume, aresample, amix, ...
│   │   │       └── subtitle/   # subtitles overlay
│   │   └── tests/
│   │
│   ├── sw-resample/            # 오디오 리샘플링 엔진
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── context.rs      # ResampleContext
│   │   │   ├── convert.rs      # 샘플 포맷 변환
│   │   │   ├── resample.rs     # 리샘플링 (sinc interpolation)
│   │   │   ├── rematrix.rs     # 채널 다운/업믹스
│   │   │   └── dither.rs       # 디더링
│   │   └── tests/
│   │
│   ├── sw-scale/               # 이미지 스케일링/색변환 엔진
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── context.rs      # ScaleContext
│   │   │   ├── scale.rs        # bilinear, bicubic, lanczos, ...
│   │   │   ├── colorspace.rs   # YUV↔RGB, BT.601/709/2020
│   │   │   ├── palette.rs      # 팔레트 변환
│   │   │   └── lut3d.rs        # 3D LUT, tone mapping
│   │   └── tests/
│   │
│   ├── av-device/              # 디바이스 입출력 (feature-gated, optional)
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── device.rs       # Device trait (input/output)
│   │   │   ├── camera.rs       # 카메라 캡처 (V4L2, AVFoundation, MSMF)
│   │   │   ├── microphone.rs   # 마이크 캡처 (ALSA, CoreAudio, WASAPI)
│   │   │   ├── screen.rs       # 화면 캡처
│   │   │   └── speaker.rs      # 오디오 출력 (ALSA, CoreAudio, WASAPI)
│   │   └── tests/
│   │
│   └── justav/                 # 통합 facade + CLI
│       ├── src/
│       │   ├── lib.rs          # pub re-export (사용자 진입점)
│       │   ├── pipeline.rs     # demux→decode→filter→encode→mux 오케스트레이터
│       │   └── bin/
│       │       ├── transcode.rs  # 트랜스코딩 CLI
│       │       ├── probe.rs      # 미디어 분석 CLI
│       │       └── play.rs       # 재생 CLI (optional, feature-gated)
│       └── tests/
│           └── integration/
│               ├── transcode_test.rs
│               ├── probe_test.rs
│               └── roundtrip_test.rs
│
├── benches/                    # criterion 벤치마크
│   ├── decode_bench.rs
│   ├── scale_bench.rs
│   └── resample_bench.rs
│
├── fuzz/                       # cargo-fuzz 타겟
│   ├── fuzz_targets/
│   │   ├── demux_mp4.rs
│   │   ├── demux_mkv.rs
│   │   ├── demux_wav.rs
│   │   ├── decode_h264.rs
│   │   ├── decode_aac.rs
│   │   ├── decode_flac.rs
│   │   └── bsf.rs
│   └── Cargo.toml
│
└── roadmap.md
```

---

## Feature Flags Strategy

```toml
# workspace Cargo.toml [features]
[features]
default = ["codec-pcm", "codec-aac", "codec-opus", "codec-h264",
           "format-mp4", "format-matroska", "format-mpegts",
           "filter-scale", "filter-aresample"]

# ── 코덱 (각각 독립) ──
codec-pcm    = []
codec-flac   = []
codec-aac    = []
codec-opus   = []
codec-mp3    = []
codec-vorbis = []
codec-h264   = []
codec-hevc   = []
codec-vp9    = []
codec-av1    = []
codec-png    = []

# ── 포맷 (각각 독립) ──
format-mp4      = []
format-matroska = []
format-mpegts   = []
format-flv      = []
format-wav      = []
format-ogg      = []
format-avi      = []
format-hls      = ["format-mpegts", "format-mp4"]
format-dash     = ["format-mp4"]

# ── 필터 ──
filter-scale     = []
filter-crop      = []
filter-aresample = []
filter-volume    = []

# ── 네트워크 ──
protocol-http = ["dep:hyper", "dep:rustls"]
protocol-tcp  = []
protocol-udp  = []
async-io      = ["dep:tokio"]

# ── 디바이스 ──
device-camera = []
device-audio  = ["dep:cpal"]

# ── 빌드 타겟 ──
simd = []           # SIMD 최적화 활성화
hw-vaapi  = []      # Linux VA-API
hw-d3d11  = []      # Windows D3D11VA
hw-vt     = []      # macOS VideoToolbox
hw-nvenc  = []      # NVIDIA NVENC/NVDEC

# ── 메타 ──
full = ["codec-pcm", "codec-flac", "codec-aac", "codec-opus", "codec-mp3",
        "codec-h264", "codec-hevc", "codec-vp9", "codec-av1",
        "format-mp4", "format-matroska", "format-mpegts", "format-flv",
        "format-wav", "format-ogg", "format-hls", "format-dash",
        "protocol-http", "async-io"]
```

---

## Thread Safety & Concurrency Model

```
┌─────────────────────────────────────────────────────┐
│  모든 public 타입의 Send/Sync 정책                    │
├──────────────────┬──────────────────────────────────┤
│ 타입              │ Send │ Sync │ 비고               │
├──────────────────┼──────┼──────┼────────────────────┤
│ Frame            │  ✓   │  ✓   │ Arc<Buffer> 기반    │
│ Packet           │  ✓   │  ✓   │ Arc<Buffer> 기반    │
│ CodecContext     │  ✓   │  ✗   │ &mut self로 보호     │
│ FormatContext    │  ✓   │  ✗   │ &mut self로 보호     │
│ FilterGraph      │  ✓   │  ✗   │ &mut self로 보호     │
│ ScaleContext     │  ✓   │  ✗   │ &mut self로 보호     │
│ ResampleContext  │  ✓   │  ✗   │ &mut self로 보호     │
│ Buffer           │  ✓   │  ✓   │ immutable + Arc     │
│ BufferPool       │  ✓   │  ✓   │ 내부 Mutex          │
│ Dict             │  ✓   │  ✓   │ 불변이면 Sync       │
│ IOContext        │  ✓   │  ✗   │ 내부 I/O 상태       │
└──────────────────┴──────┴──────┴────────────────────┘

- Context 타입은 `Send`이므로 스레드 간 이동 가능, `!Sync`이므로 동시 접근 불가.
- Pipeline 병렬화: 각 스테이지(demux, decode, filter, encode, mux)를
  별도 스레드에서 실행하고 crossbeam channel로 Frame/Packet 전달.
```

---

## Testing Strategy

모든 phase에서 구현 전 테스트를 먼저 작성한다 (TDD).

### Test Categories

```rust
#[cfg(test)] mod tests {
    // ── Positive Tests ──────────────────────────────
    // 정상 입력 → 기대 출력 확인
    #[test] fn rational_add_normal() { ... }
    #[test] fn decode_valid_h264_frame() { ... }
    #[test] fn demux_mp4_reads_all_streams() { ... }

    // ── Negative Tests ──────────────────────────────
    // 잘못된 입력 → 에러 반환, 패닉 없음
    #[test] fn rational_divide_by_zero_returns_error() { ... }
    #[test] fn decode_corrupted_packet_returns_error() { ... }
    #[test] fn demux_empty_file_returns_invalid_data() { ... }
    #[test] fn open_nonexistent_file_returns_io_error() { ... }

    // ── Edge Cases ──────────────────────────────────
    // 경계값, 오버플로우, 빈 입력
    #[test] fn rational_i32_max_no_overflow() { ... }
    #[test] fn packet_zero_size_is_valid() { ... }
    #[test] fn frame_zero_samples_audio() { ... }

    // ── Property-Based Tests ────────────────────────
    // proptest / quickcheck — 무작위 입력으로 불변조건 검증
    #[test] fn rescale_roundtrip_preserves_value() { ... }
    #[test] fn encode_decode_roundtrip_lossless() { ... }

    // ── Golden File Tests ───────────────────────────
    // 레퍼런스 구현 출력과 바이트 단위 비교
    #[test] fn demux_mp4_matches_reference_output() { ... }
    #[test] fn scale_bilinear_matches_reference_output() { ... }

    // ── Concurrency Tests ───────────────────────────
    // Send/Sync 보장, 스레드 간 이동, 데이터 레이스 없음
    #[test] fn frame_send_across_threads() { ... }
    #[test] fn buffer_pool_concurrent_access() { ... }
}
```

### Test Rules

1. **양성(Positive) : 음성(Negative) 비율** — 최소 1:1. 모든 public fn에 양쪽 다 작성.
2. **Golden file test** — 레퍼런스 출력을 `tests/fixtures/`에 저장, 바이트 비교.
3. **Fuzz target** — 외부 입력을 받는 모든 파서(demux, decode, bsf)에 fuzz 타겟 필수.
4. **No panic policy** — public API는 `Result<T, Error>` 반환. `unwrap()` 금지.
5. **Concurrency test** — `Send` 보장 타입은 스레드 간 이동 테스트, `BufferPool`은 동시 접근 테스트.
6. **Miri** — unsafe 블록은 `cargo miri test`로 UB 검증.
7. **WASM test** — `no_std` 호환 크레이트는 `wasm32-unknown-unknown` 타겟 테스트.

### CI/CD Pipeline

```
PR 생성 시:
  ├── cargo fmt --check
  ├── cargo clippy --all-features -- -D warnings
  ├── cargo test --all-features                    # 전체 테스트
  ├── cargo test --no-default-features             # 최소 빌드 테스트
  ├── cargo test --target wasm32-unknown-unknown    # WASM 빌드 (av-util)
  ├── cargo miri test -p av-util                   # UB 검증
  └── cargo doc --no-deps --all-features           # 문서 빌드

Nightly (매일):
  ├── cargo fuzz run (각 타겟 10분씩)
  ├── cargo bench                                  # 성능 회귀 감지
  └── cargo tarpaulin --all-features               # 커버리지 리포트
```

---

## Phase 0: Foundation (av-util) — DONE

> 목표: 다른 모든 크레이트가 의존할 기반 타입과 유틸리티 완성.
>
> **완료**: 18개 모듈, 299 tests, 0 clippy warnings.

### Step 0.1 — Error & Rational

| 항목 | 내용 |
|------|------|
| 구현 | `Error` enum (I/O, InvalidData, Eof, Unsupported, ...), `Result<T>` alias, `Rational` (i32/i32) |
| Positive | 사칙연산, rescale 정확도, reduce 결과 |
| Negative | 0으로 나누기, i32 오버플로우, NaN→rational |
| 의존 | 없음, `no_std` 호환 |

### Step 0.2 — Buffer & Memory

| 항목 | 내용 |
|------|------|
| 구현 | `Buffer` (Arc<[u8]> 기반 refcount), `BufferPool` |
| Positive | alloc→ref→unref 사이클, pool reuse 확인, 멀티스레드 pool 접근 |
| Negative | 0 크기 할당, pool 고갈 |
| Concurrency | `BufferPool`에 여러 스레드가 동시 get/put |
| 의존 | Step 0.1 |

### Step 0.3 — Frame & Side Data

| 항목 | 내용 |
|------|------|
| 구현 | `Frame` (video: planes+linesize, audio: samples+channels), `FrameSideData` |
| Positive | video frame 생성/복사, audio frame 생성/복사, side data 추가/조회 |
| Negative | 잘못된 pixfmt, 0 크기 해상도, side data 없는 타입 조회 |
| Concurrency | Frame을 다른 스레드로 Send |
| 의존 | Step 0.2 |

### Step 0.4 — Pixel/Sample Formats, Channel Layout

| 항목 | 내용 |
|------|------|
| 구현 | `PixelFormat` enum + descriptor, `SampleFormat`, `ChannelLayout` |
| Positive | 포맷별 bpp/planar 조회, 레이아웃 비교, 알려진 포맷 전수 검증 |
| Negative | 알 수 없는 포맷 ID, 잘못된 채널 수 |
| 의존 | Step 0.1, `no_std` 호환 |

### Step 0.5 — Timestamp Management

| 항목 | 내용 |
|------|------|
| 구현 | `Timestamp` (i64 + timebase), 불연속(discontinuity) 감지, 타임베이스 변환 |
| Positive | rescale 정확도, compare 정확도, 불연속 감지 임계값 |
| Negative | NOPTS 값 처리, 오버플로우, 음수 타임스탬프 |
| 의존 | Step 0.1 |

### Step 0.6 — Dict, Log, Hash, Base64, FIFO

| 항목 | 내용 |
|------|------|
| 구현 | `Dict` (ordered map), 로깅 (tracing 연동), 해시, base64, 링 버퍼 |
| Positive | dict CRUD, hash digest 정합성 (RFC 테스트벡터), base64 roundtrip |
| Negative | 빈 dict 조회, 잘못된 해시 이름, 잘린 base64 |
| 의존 | Step 0.1 |

### Step 0.7 — Image/Sample Utilities

| 항목 | 내용 |
|------|------|
| 구현 | `imgutils` (linesize, buffer size, copy, fill_black), `samples` (alloc, copy, silence) |
| Positive | 다양한 포맷별 linesize 계산, black fill 검증 |
| Negative | 음수 크기, 미지원 포맷, 0x0 해상도 |
| 의존 | Step 0.3, 0.4 |

### Step 0.8 — Option System

| 항목 | 내용 |
|------|------|
| 구현 | 타입 안전 옵션 파싱 (`key=value`), 범위 검증, 기본값, 열거형 옵션 |
| Positive | int/float/string/bool/rational 옵션 set/get, 기본값 적용 |
| Negative | 범위 밖 값, 잘못된 타입, 존재하지 않는 키 |
| 의존 | Step 0.1, 0.6 |

**Phase 0 완료 기준**: `cargo test -p av-util` 전체 통과, 커버리지 ≥ 80%, `cargo miri test` 통과.

---

## Phase 1: Codec Core (av-codec) — DONE (FLAC 제외)

> 목표: 인코딩/디코딩 프레임워크 + 첫 번째 코덱 (PCM, FLAC).
>
> **완료**: 11개 모듈, 68 tests. PCM 5종 encode/decode roundtrip 통과, SRT 자막 파싱 통과.
> FLAC 디코더는 별도 이슈로 분리 (복잡도 높음).

### Step 1.1 — Packet & CodecParameters

| 항목 | 내용 |
|------|------|
| 구현 | `Packet` (data + side_data + pts/dts), `CodecParameters` |
| Positive | 패킷 생성/복제/rescale_ts, 파라미터 복사 |
| Negative | 빈 패킷 side_data 조회, 잘못된 codec_id |
| 의존 | Phase 0 |

### Step 1.2 — Codec Trait & Registry

| 항목 | 내용 |
|------|------|
| 구현 | `trait Decoder`, `trait Encoder`, `CodecId` enum, `codec_registry` |
| Positive | 등록→조회→타입 확인, feature flag별 가용 코덱 |
| Negative | 미등록 코덱 조회, 디코더를 인코더로 사용 시도 |
| 의존 | Step 1.1 |

### Step 1.3 — CodecContext & Send/Receive API

| 항목 | 내용 |
|------|------|
| 구현 | `CodecContext` (open/send_packet/receive_frame/send_frame/receive_packet/flush) |
| Positive | open→send→receive 사이클 (PCM으로 검증) |
| Negative | 미초기화 상태에서 send, 잘못된 포맷 프레임 전송, double open |
| Concurrency | CodecContext를 다른 스레드로 move |
| 의존 | Step 1.2 |

### Step 1.4 — Error Resilience Framework

| 항목 | 내용 |
|------|------|
| 구현 | 에러 복구 정책 enum (Skip, Conceal, Fail), 프레임 은닉(concealment) 인터페이스 |
| Positive | 손상 패킷 → 이전 프레임 복사(concealment) 후 계속 디코딩 |
| Negative | 모든 프레임 손상 시 → 최종 에러 반환 |
| 의존 | Step 1.3 |

### Step 1.5 — PCM Codecs

| 항목 | 내용 |
|------|------|
| 구현 | PCM s16le/s16be/f32le/f32be/u8 인코더+디코더 |
| Positive | encode→decode roundtrip 바이트 일치 |
| Negative | 잘못된 sample_rate, 0 채널 |
| 의존 | Step 1.3 |

### Step 1.6 — FLAC Decoder

| 항목 | 내용 |
|------|------|
| 구현 | FLAC 디코더 (무손실이므로 golden test 용이) |
| Positive | FLAC 파일 디코딩 → 원본 PCM과 바이트 비교 |
| Negative | 잘린 프레임, 잘못된 streaminfo, CRC 오류 |
| Fuzz | `fuzz_targets/decode_flac.rs` |
| 의존 | Step 1.3 |

### Step 1.7 — Bitstream Filters

| 항목 | 내용 |
|------|------|
| 구현 | `BsfContext`, `trait BitstreamFilter`, `BsfChain` |
| Positive | null filter passthrough, h264_mp4toannexb (Phase 3에서) |
| Negative | 미초기화 send, 잘못된 codec_id |
| Fuzz | `fuzz_targets/bsf.rs` |
| 의존 | Step 1.1 |

### Step 1.8 — Parser

| 항목 | 내용 |
|------|------|
| 구현 | `Parser` trait, `ParserContext` |
| Positive | H.264 NAL 단위 분리 (Phase 3에서 구현체 추가) |
| Negative | 빈 입력, 알 수 없는 codec_id |
| 의존 | Step 1.1 |

### Step 1.9 — Subtitle Framework

| 항목 | 내용 |
|------|------|
| 구현 | `SubtitleDecoder` trait, `SubtitleFrame` (text + timing), SRT 파서 |
| Positive | SRT 파일 파싱, 타이밍 정확도 |
| Negative | 잘못된 시간 포맷, 빈 자막, 중첩 타이밍 |
| 의존 | Step 1.2 |

**Phase 1 완료 기준**: PCM roundtrip + FLAC 디코딩 golden test + SRT 파싱 통과.

---

## Phase 2: Container Core (av-format) — DONE (Matroska 제외)

> 목표: 먹싱/디먹싱 프레임워크 + 첫 번째 컨테이너 (WAV, Matroska).
>
> **완료**: 11개 모듈, 53 tests. WAV roundtrip (s16le/u8/f32le) 통과.
> Matroska EBML 파서는 별도 이슈로 분리 (복잡도 높음).

### Step 2.1 — IOContext (Sync)

| 항목 | 내용 |
|------|------|
| 구현 | `IOContext` — `std::io::{Read, Write, Seek}` 기반 + 버퍼링 + endian helpers |
| Positive | 파일/메모리 백엔드 읽기·쓰기, endian 정합성 |
| Negative | EOF 넘어서 읽기, seek 범위 초과, non-seekable 소스에서 seek 시도 |
| 의존 | Phase 0 |

### Step 2.2 — Protocol Trait & File Protocol

| 항목 | 내용 |
|------|------|
| 구현 | `trait Protocol` (open/read/write/seek/close), `FileProtocol`, `PipeProtocol` |
| Positive | 파일 열기/읽기/쓰기/시크, stdin 파이프 읽기 |
| Negative | 존재하지 않는 파일, 권한 없는 파일, 파이프에서 seek |
| 의존 | Step 2.1 |

### Step 2.3 — Demuxer Trait & FormatContext (Input)

| 항목 | 내용 |
|------|------|
| 구현 | `trait Demuxer`, `FormatContext::open_input`, `read_frame`, probe 시스템 |
| Positive | open→read_frame 루프→close, 스트림 정보 정확성 |
| Negative | 빈 파일, 잘못된 매직넘버, probe score 0 |
| 의존 | Step 2.1, 2.2 |

### Step 2.4 — Muxer Trait & FormatContext (Output)

| 항목 | 내용 |
|------|------|
| 구현 | `trait Muxer`, `FormatContext::open_output`, `write_header/frame/trailer` |
| Positive | header→packets→trailer 시퀀스, 출력 파일 유효성 |
| Negative | 스트림 없이 write_header, trailer 전에 close |
| 의존 | Step 2.1, 2.2 |

### Step 2.5 — Interleaving & Timestamp Management

| 항목 | 내용 |
|------|------|
| 구현 | DTS 기반 인터리빙, 타임베이스 변환, 불연속 처리 |
| Positive | A/V 패킷 DTS 순서 보장, 타임베이스 변환 정확도 |
| Negative | 역방향 DTS, 거대 갭(불연속), 타임베이스 0 |
| 의존 | Step 2.4, Phase 0 Step 0.5 |

### Step 2.6 — Metadata, Chapters, Attachments

| 항목 | 내용 |
|------|------|
| 구현 | 구조화 메타데이터 (title, artist, ...), 챕터 (start/end/title), 첨부파일 (앨범아트 등) |
| Positive | 메타데이터 읽기/쓰기 roundtrip, 챕터 정확도 |
| Negative | 빈 메타데이터, 중첩 챕터, 거대 첨부파일 |
| 의존 | Step 2.3, 2.4 |

### Step 2.7 — WAV Demuxer/Muxer

| 항목 | 내용 |
|------|------|
| 구현 | RIFF/WAV 읽기·쓰기 (가장 단순한 컨테이너) |
| Positive | WAV 파일 demux→PCM decode→encode→mux roundtrip |
| Negative | 잘린 RIFF 헤더, fmt 청크 누락, 비표준 chunk |
| Fuzz | `fuzz_targets/demux_wav.rs` |
| 의존 | Step 2.3, 2.4, Phase 1 (PCM) |

### Step 2.8 — Matroska/WebM Demuxer

| 항목 | 내용 |
|------|------|
| 구현 | EBML 파서 + Matroska 디먹서 (읽기 전용) |
| Positive | mkv/webm 파일 스트림 탐지, 패킷 읽기, 시크, 메타데이터/챕터 |
| Negative | 잘못된 EBML 헤더, 미지원 코덱, 거대한 element size |
| Fuzz | `fuzz_targets/demux_mkv.rs` |
| 의존 | Step 2.3 |

### Step 2.9 — Seek & Index

| 항목 | 내용 |
|------|------|
| 구현 | 타임스탬프 기반 시크, 인덱스 빌드 |
| Positive | 키프레임 시크 정확도, 양방향 시크 |
| Negative | 범위 밖 타임스탬프, 인덱스 없는 포맷, non-seekable 소스 |
| 의존 | Step 2.3 |

**Phase 2 완료 기준**: WAV roundtrip + MKV 디먹싱 golden test 통과.

---

## Phase 3: Codec Parsers & Subtitle Codecs — DONE

> 목표: 코덱별 비트스트림 파서 + 자막 코덱 완전 구현.
>
> **완료**: 67 tests 추가 (av-codec 총 137). H.264 NAL/SPS, AAC ADTS, Opus TOC, ASS, WebVTT 파서 구현.
>
> 핵심 코덱의 실제 디코딩 알고리즘(IDCT, MDCT, 모션 보상 등)은
> Phase 3b에서 별도 진행. 이 Phase에서는 **파서/헤더 분석**까지만.

### Step 3.1 — H.264 NAL Parser & Header Analysis

| 항목 | 내용 |
|------|------|
| 구현 | NAL 유닛 분리 (Annex B start code + AVCC length-prefix), NAL 타입 분류, SPS/PPS 기본 파싱 (해상도, 프로파일, 레벨 추출) |
| Positive | Annex B 스트림에서 NAL 분리, SPS에서 해상도 추출 |
| Negative | 잘린 NAL, start code 없는 데이터, 잘못된 SPS |
| 범위 | 파싱만. 실제 slice 디코딩은 Phase 3b |

### Step 3.2 — AAC ADTS Parser & Header Analysis

| 항목 | 내용 |
|------|------|
| 구현 | ADTS 헤더 파싱 (sync word, 프로파일, 샘플레이트, 채널, 프레임 크기), ADTS 프레임 분리 |
| Positive | ADTS 스트림에서 프레임 분리, 헤더 필드 정확도 |
| Negative | sync word 불일치, 잘린 프레임, 잘못된 샘플레이트 인덱스 |
| 범위 | 파싱만. 실제 MDCT/TNS 디코딩은 Phase 3b |

### Step 3.3 — Opus Packet Parser

| 항목 | 내용 |
|------|------|
| 구현 | TOC 바이트 파싱 (모드, 대역폭, 프레임 수), 패킷 구조 분석 |
| Positive | TOC에서 모드/대역폭/프레임 크기 추출 |
| Negative | 잘못된 TOC 바이트, 패킷 크기 불일치 |
| 범위 | 파싱만. SILK/CELT 디코딩은 Phase 3b |

### Step 3.4 — Subtitle Codecs (ASS, WebVTT)

| 항목 | 내용 |
|------|------|
| 구현 | ASS/SSA 파서 (헤더 섹션, 이벤트, 스타일 태그), WebVTT 파서 (큐, 타이밍, 포지셔닝) |
| Positive | 스타일 태그 파싱, 타이밍 정확도, 다중 라인, 이벤트 순서 |
| Negative | 잘못된 태그 중첩, UTF-8 외 인코딩, 빈 큐 |
| SRT | Phase 1에서 완료 |

### Step 3.5 — Codec Registration & Stub Decoders

| 항목 | 내용 |
|------|------|
| 구현 | H.264/AAC/Opus/VP9/AV1 디코더 스텁을 CodecRegistry에 등록, 파서 결과를 CodecParameters로 변환하는 헬퍼 |
| Positive | 등록된 코덱 조회, 파서→CodecParameters 변환 |
| Negative | 미구현 디코더 호출 시 `Error::Unsupported` 반환 |

**Phase 3 완료 기준**: H.264 NAL 파서 + AAC ADTS 파서 + ASS/WebVTT 자막 파싱 통과.

---

## Phase 4: MP4 Container & First E2E Transcode

> 목표: MP4 demux/mux로 실제 트랜스코딩 파이프라인 완성.

### Step 4.1 — MP4/MOV Demuxer (일반 + fragmented)

| 항목 | 내용 |
|------|------|
| 구현 | ISO BMFF 파서 (ftyp, moov, mdat, stbl, stts, stsc, stsz, stco, ctts) + fragmented MP4 (moof, traf, tfhd, trun) |
| Positive | 일반 MP4 + fMP4 스트림 탐지, 패킷 추출, 시크, 메타데이터 |
| Negative | 잘못된 box 크기, 순환 참조, 누락된 moov |
| Fuzz | `fuzz_targets/demux_mp4.rs` |

### Step 4.2 — MP4 Muxer

| 항목 | 내용 |
|------|------|
| 구현 | MP4 쓰기 (moov-at-end + faststart 옵션) + fMP4 모드 |
| Positive | 출력 MP4 유효성 검증, 재생 가능 확인, > 4GB 파일 (64-bit offsets) |
| Negative | 스트림 없이 mux, 타임스탬프 미설정 |

### Step 4.3 — E2E Transcode Pipeline

| 항목 | 내용 |
|------|------|
| 구현 | `Pipeline`: demux→decode→(filter)→encode→mux 오케스트레이터 |
| Positive | MP4→MP4 트랜스코딩, 출력 재생 가능, A/V sync 검증 |
| Negative | 코덱 불일치, 타임스탬프 불연속, 중간 EOF |
| Golden | 레퍼런스 출력과 메타데이터 비교 |

**Phase 4 완료 기준**: `input.mp4` → justav transcode → `output.mp4` 재생 가능.

---

## Phase 5: Filter Graph (av-filter)

> 목표: 비디오/오디오 필터 체인 프레임워크.

### Step 5.1 — Graph & Link & Pad

| 항목 | 내용 |
|------|------|
| 구현 | `FilterGraph`, `Filter` trait, `FilterLink`, 포맷 네고시에이션 |
| Positive | graph 생성→config→실행, 직렬/분기 토폴로지 |
| Negative | 연결되지 않은 패드, 순환 그래프, 포맷 불일치 |

### Step 5.2 — Buffer Source/Sink

| 항목 | 내용 |
|------|------|
| 구현 | `BufferSource`, `BufferSink` |
| Positive | 프레임 입력→통과→출력 (null filter) |
| Negative | EOF 후 추가 입력, 초기화 전 get_frame |

### Step 5.3 — Essential Video Filters

| 항목 | 내용 |
|------|------|
| 구현 | `scale` (sw-scale 연동), `crop`, `pad`, `fps`, `overlay`, `subtitles` (자막 burn-in) |
| Positive | 각 필터 출력 해상도/포맷 검증 |
| Negative | 0 크기 crop, 음수 pad, scale 0x0 |

### Step 5.4 — Essential Audio Filters

| 항목 | 내용 |
|------|------|
| 구현 | `aresample` (sw-resample 연동), `volume`, `amix` |
| Positive | 리샘플 정확도, 볼륨 dB 정확도 |
| Negative | 0 sample_rate, 볼륨 음수 |

### Step 5.5 — Graph String Parser

| 항목 | 내용 |
|------|------|
| 구현 | `"[0:v]scale=1280:720[out]"` 같은 문자열 파싱 |
| Positive | 복합 필터 체인 파싱, 세미콜론 구분, 명명된 패드 |
| Negative | 문법 오류, 미등록 필터 이름, 닫히지 않은 괄호 |

**Phase 5 완료 기준**: scale+aresample+subtitles 포함 필터 그래프 E2E 동작.

---

## Phase 6: Scaling & Resampling (sw-scale, sw-resample)

> 목표: 이미지 스케일링과 오디오 리샘플링 엔진.

### Step 6.1 — sw-scale Core

| 항목 | 내용 |
|------|------|
| 구현 | bilinear, bicubic, lanczos 스케일러 + 픽셀포맷 변환 (YUV↔RGB) |
| Positive | 다양한 해상도/포맷 조합, 레퍼런스 출력 대비 PSNR 비교 |
| Negative | 0 크기, 미지원 포맷 조합, 같은 포맷 noop 확인 |

### Step 6.2 — sw-scale Color Management

| 항목 | 내용 |
|------|------|
| 구현 | BT.601/709/2020 매트릭스, HDR tone mapping, 3D LUT |
| Positive | 색공간 변환 정확도 (CIEDE2000 < 1.0) |
| Negative | 잘못된 primaries, 범위 밖 값 클램핑 |

### Step 6.3 — sw-resample Core

| 항목 | 내용 |
|------|------|
| 구현 | 리샘플링 (sinc interpolation), 포맷 변환 (int↔float), 채널 리매트릭싱 |
| Positive | 44100→48000 변환 후 스펙트럼 비교, 5.1→stereo 다운믹스 |
| Negative | 0 sample_rate, 잘못된 채널 레이아웃 |

**Phase 6 완료 기준**: scale + resample 출력이 레퍼런스 대비 오차 허용 범위 내.

---

## Phase 7: Network & Streaming

> 목표: HTTP/TCP/UDP 프로토콜 + HLS/DASH 스트리밍 지원.

### Step 7.1 — HTTP/HTTPS Protocol

| 항목 | 내용 |
|------|------|
| 구현 | `HttpProtocol` (hyper + rustls 기반), Range 요청, redirect, auth |
| Positive | HTTP URL demux, Range seek, HTTPS 인증서 검증 |
| Negative | 404 응답, 연결 타임아웃, 잘못된 인증서 |
| 의존 | Phase 2 Step 2.2 |

### Step 7.2 — TCP/UDP Protocol

| 항목 | 내용 |
|------|------|
| 구현 | `TcpProtocol`, `UdpProtocol` (tokio 기반) |
| Positive | TCP 스트림 읽기, UDP 멀티캐스트 수신 |
| Negative | 연결 거부, 패킷 손실 (UDP) |

### Step 7.3 — Async I/O Layer

| 항목 | 내용 |
|------|------|
| 구현 | `AsyncIOContext` (tokio `AsyncRead`/`AsyncWrite`/`AsyncSeek` 기반) |
| Positive | async demux 동작, non-seekable 소스 처리 |
| Negative | 타임아웃, 읽기 중 연결 끊김 |
| 의존 | Step 7.1, 7.2 |

### Step 7.4 — HLS Demuxer/Muxer

| 항목 | 내용 |
|------|------|
| 구현 | M3U8 파서, TS 세그먼트 연결, 라이브/VOD 모드 |
| Positive | HLS URL demux, 세그먼트 전환, 대역폭 적응 (adaptive) |
| Negative | 잘못된 M3U8, 세그먼트 404, 불연속 시퀀스 |
| 의존 | Step 7.1, Phase 2 (MPEG-TS) |

### Step 7.5 — DASH Demuxer/Muxer

| 항목 | 내용 |
|------|------|
| 구현 | MPD 파서, fMP4 세그먼트, 라이브/VOD |
| Positive | DASH URL demux, period 전환 |
| Negative | 잘못된 MPD, 미지원 프로파일 |
| 의존 | Step 7.1, Phase 4 (fMP4) |

**Phase 7 완료 기준**: HTTP URL에서 HLS/DASH 스트림 디먹싱 동작.

---

## Phase 8: Extended Formats & Codecs

> 목표: 실용성 확대 — 추가 컨테이너와 코덱.

### Step 8.1 — Additional Containers

| 우선순위 | 포맷 | 타입 |
|---------|------|------|
| 높음 | MPEG-TS | demux + mux |
| 높음 | FLV | demux + mux |
| 중간 | OGG | demux + mux |
| 중간 | AVI | demux |

### Step 8.2 — Additional Codecs

| 우선순위 | 코덱 | 타입 |
|---------|------|------|
| 높음 | HEVC/H.265 | decode |
| 높음 | VP8 | decode |
| 중간 | MP3 (MPEG Layer 3) | decode |
| 중간 | Vorbis | decode |
| 중간 | FLAC encoder | encode |
| 낮음 | PNG | decode + encode |

**Phase 8 완료 기준**: 상위 10개 컨테이너/코덱 지원.

---

## Phase 9: Performance & Hardware

> 목표: 프로덕션 수준 성능.

### Step 9.1 — SIMD Optimization

| 항목 | 내용 |
|------|------|
| 구현 | `std::arch` (x86 SSE/AVX2, ARM NEON) + portable SIMD fallback |
| 타겟 | 스케일링 inner loop, 디코더 IDCT/MC, 리샘플링 FIR, 색공간 변환 |
| 테스트 | SIMD 경로와 scalar 경로 결과 bit-exact 비교 |

### Step 9.2 — Multithreading

| 항목 | 내용 |
|------|------|
| 구현 | 슬라이스 스레딩 (rayon) — 프레임 내 병렬 |
| 구현 | 프레임 스레딩 — 파이프라인 병렬 |
| 구현 | Pipeline 스테이지 스레드 분리 (crossbeam channel) |
| 테스트 | 단일 스레드 vs 멀티 스레드 결과 일치 확인, 데드락 감지 |

### Step 9.3 — Hardware Acceleration

| 항목 | 내용 |
|------|------|
| 구현 | `hwcontext.rs` (av-util에 추가): `HwDeviceType` enum, `trait HwDeviceContext`, `trait HwFramesContext` |
| 구현 | `trait HwDecoder` / `trait HwEncoder` 추상화 |
| 백엔드 | VAAPI (Linux), D3D11VA (Windows), VideoToolbox (macOS), NVDEC/NVENC |
| 단계 | hwcontext trait 정의 → 1개 백엔드 PoC → 나머지 확장 |
| 테스트 | HW decode 결과 vs SW decode 결과 PSNR 비교 |

### Step 9.4 — Benchmarks & Profiling

| 항목 | 내용 |
|------|------|
| 구현 | `criterion` 벤치마크, flamegraph 프로파일링 |
| 최소 목표 | 레퍼런스 C 구현 대비 ≥ 80% throughput |
| 항목 | decode (H.264 1080p), scale (1080p→720p), resample (48k→44.1k) |

**Phase 9 완료 기준**: 주요 경로 SIMD 최적화, 레퍼런스 대비 ≥ 80% 성능.

---

## Phase 10: Device I/O (av-device)

> 목표: 카메라/마이크/스크린 입출력. 전체 optional, feature-gated.

### Step 10.1 — Device Trait & Enumeration

| 항목 | 내용 |
|------|------|
| 구현 | `trait InputDevice`, `trait OutputDevice`, 디바이스 열거 |
| Positive | 사용 가능한 디바이스 목록 조회 |
| Negative | 디바이스 없는 환경에서 열거 |

### Step 10.2 — Camera Capture

| 항목 | 내용 |
|------|------|
| 구현 | V4L2 (Linux), AVFoundation (macOS), MediaFoundation (Windows) |
| Positive | 카메라 열기→프레임 캡처→닫기 |
| Negative | 디바이스 사용 중, 미지원 해상도 |

### Step 10.3 — Audio Capture/Playback

| 항목 | 내용 |
|------|------|
| 구현 | `cpal` 크레이트 기반 크로스 플랫폼 오디오 |
| Positive | 마이크 녹음→프레임 생성, 프레임→스피커 재생 |
| Negative | 디바이스 없는 환경, 버퍼 언더런 |

**Phase 10 완료 기준**: 카메라+마이크 캡처 → 인코딩 → 파일 저장 E2E.

---

## Phase 11: CLI Tools & Polish

### Step 11.1 — justav transcode (트랜스코딩 CLI)

| 항목 | 내용 |
|------|------|
| 구현 | `clap` 기반 CLI |
| 문법 | `justav transcode -i input.mp4 -c:v h264 -c:a aac output.mp4` |
| 기능 | 진행률 표시, 필터 그래프 지정 (`-vf`, `-af`), 스트림 매핑 (`-map`) |

### Step 11.2 — justav probe (미디어 분석 CLI)

| 항목 | 내용 |
|------|------|
| 구현 | JSON/텍스트 출력, 스트림/포맷/패킷/프레임 분석 |
| 문법 | `justav probe -show_streams -show_format -of json input.mp4` |

### Step 11.3 — justav play (재생 CLI, optional)

| 항목 | 내용 |
|------|------|
| 구현 | `wgpu` (렌더링) + `cpal` (오디오) 기반 |
| feature | `feature = "player"` |

### Step 11.4 — API Stabilization & Docs

| 항목 | 내용 |
|------|------|
| 구현 | public API `#[doc]` 완성, `docs.rs` 배포 |
| 산출물 | CHANGELOG, MIGRATION guide, 예제 코드 |

---

## Phase 12: Codec Algorithm Implementation (전체 프레임워크 완성 후 진행)

> 목표: 실제 디코딩/인코딩 알고리즘 구현. 각 코덱은 독립적으로 진행 가능.
> Phase 4~11의 프레임워크(파이프라인, 필터, 스케일링, CLI 등)가 완성된 후 진행.
> 각 코덱은 수만 줄 규모이므로 별도 이슈/PR로 관리.

### Step 12.1 — H.264 Decoder (Full)

| 항목 | 내용 |
|------|------|
| 구현 | Exp-Golomb, slice header, 참조 프레임 관리, intra/inter 예측, IDCT, deblocking filter |
| 스펙 | ITU-T H.264 (ISO/IEC 14496-10) |
| 에러 복구 | 손상 NAL 건너뛰기, 참조 프레임 누락 시 concealment |
| Positive | conformance test vectors, 레퍼런스 출력 대비 PSNR ≥ 60dB |
| Fuzz | `fuzz_targets/decode_h264.rs` |

### Step 12.2 — AAC Decoder (Full)

| 항목 | 내용 |
|------|------|
| 구현 | AAC-LC: Huffman decoding, inverse quantization, MDCT, TNS, PNS, MS, IS, 윈도잉 |
| 스펙 | ISO/IEC 13818-7 (MPEG-2 AAC), ISO/IEC 14496-3 (MPEG-4 AAC) |
| Positive | 다양한 bitrate/채널 구성 디코딩, 레퍼런스 비교 |
| Negative | 미지원 프로파일 (HE-AAC는 후순위) |
| Fuzz | `fuzz_targets/decode_aac.rs` |

### Step 12.3 — Opus Decoder (Full)

| 항목 | 내용 |
|------|------|
| 구현 | SILK 디코더 + CELT 디코더 + 하이브리드 모드 |
| 스펙 | RFC 6716 |
| Positive | 다양한 bitrate, 채널 수, 프레임 크기 |

### Step 12.4 — VP9 / AV1 Decoder

| 항목 | 내용 |
|------|------|
| 구현 | VP9 디코더 → AV1 디코더 순차 진행 |
| 스펙 | VP9 Bitstream Spec, AV1 Spec (AOMedia) |
| Positive | 프로파일 0/2, 다양한 해상도 |

### Step 12.5 — 인코더 (H.264, AAC)

| 항목 | 내용 |
|------|------|
| 구현 | H.264 Baseline encoder (CBR), AAC-LC encoder |
| Positive | encode→decode roundtrip PSNR 확인 |
| Negative | 미지원 해상도, 잘못된 bitrate |

**Phase 12 완료 기준**: H.264+AAC 디코딩이 레퍼런스 대비 PSNR ≥ 60dB.

---

## Milestone Summary

| Phase | 산출물 | 핵심 검증 |
|-------|--------|----------|
| **0** | av-util 크레이트 | ~~단위 테스트 + miri 통과~~ **DONE** (299 tests) |
| **1** | av-codec + PCM/FLAC/SRT | ~~FLAC golden test + SRT 파싱~~ **DONE** (68 tests, FLAC 별도) |
| **2** | av-format + WAV/MKV | ~~WAV roundtrip + MKV demux~~ **DONE** (53 tests, MKV 별도) |
| **3** | 코덱 파서 + ASS/WebVTT 자막 | ~~H.264 NAL + AAC ADTS + 자막 파싱 통과~~ **DONE** (67 tests) |
| **4** | MP4 (일반+fMP4) + E2E transcode | 출력 MP4 재생 가능 |
| **5** | av-filter 그래프 | scale+aresample+subtitles E2E |
| **6** | sw-scale, sw-resample | 레퍼런스 대비 오차 범위 내 |
| **7** | 네트워크 + HLS/DASH | HTTP URL에서 스트림 디먹싱 |
| **8** | 추가 포맷/코덱 | top-10 포맷/코덱 지원 |
| **9** | SIMD, threading, HW 가속 | 레퍼런스 대비 ≥ 80% 성능 |
| **10** | av-device | 카메라+마이크 캡처 E2E |
| **11** | CLI 도구 + docs | 사용자 배포 가능 |
| **12** | 코덱 알고리즘 (H.264/AAC/Opus/VP9/AV1) | H.264+AAC PSNR ≥ 60dB |
