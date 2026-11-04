# Third-party notices

Parler is MIT-licensed (see [LICENSE](LICENSE)). It builds on the following
open-source work.

## Handy

Parts of Parler are adapted from [Handy](https://github.com/cjpais/Handy):
the non-activating overlay window (`src-tauri/src/overlay.rs`) and the
keyboard-layout-aware paste shortcut (`src-tauri/src/paste.rs`). Microphone
capture uses [`handy-recorder`](https://crates.io/crates/handy-recorder),
extracted from Handy and published under the MIT licence.

```
MIT License

Copyright (c) 2025 CJ Pais

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## transcribe.cpp and ggml

Transcription on this computer runs on
[transcribe.cpp](https://github.com/handy-computer/transcribe.cpp), through
its Rust bindings ([`transcribe-cpp`](https://crates.io/crates/transcribe-cpp)),
built into Parler (macOS) or shipped next to it (Windows, Linux). It includes
[ggml](https://github.com/ggml-org/ggml), [miniz](https://github.com/richgel999/miniz)
and code translated from [Silero VAD](https://github.com/snakers4/silero-vad),
all under the MIT licence.

```
MIT License

Copyright (c) 2026 The transcribe.cpp authors
Copyright (c) 2023-2026 The ggml authors
Copyright 2013-2014 RAD Game Tools and Valve Software
Copyright 2010-2014 Rich Geldreich and Tenacious Software LLC
Copyright (c) 2020-present Silero Team

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Speech models

Parler doesn't ship a model: one is downloaded when transcription on this
computer is turned on, from the GGUF conversions published by
[handy-computer](https://huggingface.co/handy-computer) on Hugging Face.

- **Parakeet Ultra**
  ([handy-computer/parakeet-ultra-gguf](https://huggingface.co/handy-computer/parakeet-ultra-gguf),
  Q8_0) is [Moondream](https://moondream.ai)'s post-trained
  [moondream/parakeet-ultra](https://huggingface.co/moondream/parakeet-ultra),
  built on NVIDIA's
  [parakeet-tdt-0.6b-v3](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3).
  Licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/);
  converted to GGUF and quantized by handy-computer, otherwise unchanged.
- **Whisper Large v3 Turbo**
  ([handy-computer/whisper-large-v3-turbo-gguf](https://huggingface.co/handy-computer/whisper-large-v3-turbo-gguf),
  Q5_K_M) is OpenAI's
  [whisper-large-v3-turbo](https://huggingface.co/openai/whisper-large-v3-turbo),
  released under the MIT licence (the conversion is published under
  Apache-2.0); converted to GGUF and quantized by handy-computer.
