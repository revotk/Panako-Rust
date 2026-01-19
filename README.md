# Panako Rust - Acoustic Fingerprinting

Port completo del algoritmo de fingerprinting acústico **Panako** de Java a Rust. Genera fingerprints de audio/video y realiza matching contra una base de datos.

## 🎯 Características

### ✅ Completamente Implementado

- **Generación de Fingerprints** (`fpgen`)
  - Soporte de audio: WAV, MP3, FLAC, OGG
  - Soporte de video: MP4, MKV, AVI, MOV, WEBM, MPEG-TS
  - Extracción automática de audio desde video
  - Resampling a 16kHz mono
  - **Monitor mode**: Segmentación automática para archivos >25s
  - Salida en formato `.fp` binario + JSON

- **Matching de Fingerprints** (`fpmatcher`)
  - Índice invertido para búsqueda rápida
  - Carga paralela de base de datos (rayon)
  - Alineación temporal automática
  - Detección de velocidad (time_factor)
  - Detección de pitch (frequency_factor)
  - Cálculo de cobertura temporal
  - Filtrado automático de matches de baja calidad
  - Salida JSON estructurada con conteo de detecciones
  - **Soporte transparente** para archivos segmentados

- **Sin Dependencias Runtime**
  - Binarios standalone (~5MB total)
  - Decoders puros Rust (excepto FFmpeg para TS)
  - No requiere Java, Python, ni otras dependencias

### 🚧 En Desarrollo

- **Coverage Mejorado**
  - Cálculo de `ref_coverage` (% de referencia matched)
  - Requiere almacenar duración de archivos de referencia

- **Integración PostgreSQL** (Planeado)
  - Almacenamiento de fingerprints en DB
  - Query desde base de datos
  - Historial de matches
  - Ver `implementation_plan.md` para detalles

## 📦 Instalación

### Compilar desde Código

```bash
# Clonar repositorio
git clone <repo-url>
cd dpt

# Compilar en release
cargo build --release

# Binarios en target/release/
# - fpgen.exe (4.87 MB)
# - fpmatcher.exe (2.02 MB)
```

### Requisitos

- **Rust 1.70+** para compilar
- **FFmpeg** (opcional, solo para archivos `.ts`)
  - Windows: https://ffmpeg.org/download.html
  - Linux: `sudo apt install ffmpeg`
  - macOS: `brew install ffmpeg`

## 🚀 Uso Rápido

### 1. Generar Fingerprints

```bash
# Audio
fpgen song.mp3 ./fingerprints/

# Video (extracción automática)
fpgen video.mp4 ./fingerprints/

# MPEG-TS (requiere FFmpeg)
fpgen video.ts ./fingerprints/

# Directorio completo
for file in ./audio/*.mp3; do fpgen "$file" ./fingerprints/; done
```

**Output:**
```json
{
  "status": "success",
  "input_file": "song.mp3",
  "output_file": "./fingerprints/song.fp",
  "duration_seconds": 180.5,
  "num_fingerprints": 2241,
  "processing_time_seconds": 3.58
}
```

### Monitor Mode (Archivos Largos)

Usa la bandera `-m` o `--monitor` para habilitar segmentación automática en archivos >25 segundos:

```bash
# Sin -m: siempre procesa como archivo único (sin importar duración)
fpgen broadcast_3h.mp3 ./fingerprints/

# Con -m: segmenta archivos >25s en chunks de 25s con 5s de traslape
fpgen broadcast_3h.mp3 ./monitoring/ --monitor
# o
fpgen broadcast_3h.mp3 ./monitoring/ -m
```

**Output con -m (segmentación):**
```json
{
  "status": "success",
  "input_file": "broadcast_3h.mp3",
  "output_file": "./monitoring/broadcast_3h.fp",
  "duration_seconds": 10800.0,
  "num_fingerprints": 48532,
  "num_segments": 540,
  "segment_duration_s": 25.0,
  "overlap_duration_s": 5.0,
  "processing_time_seconds": 125.3
}
```

**Ventajas:**
- ✅ Un solo archivo `.fp` para todo el audio
- ✅ Timestamps absolutos correctos en fingerprints
- ✅ No pierde matches en bordes de segmentos (traslape de 5s)
- ✅ Matcher funciona transparentemente
- ✅ Ideal para broadcasts, grabaciones largas, películas

### 2. Buscar Matches

```bash
# Crear base de datos
fpgen song1.mp3 ./db/
fpgen song2.mp3 ./db/
fpgen song3.mp3 ./db/

# Query
fpgen query.mp3 ./query/
fpmatcher ./db/ ./query/query.fp
```

**Output:**
```json
{
  "query_path": "./query/query.fp",
  "detections": 2,
  "results": [
    {
      "query_path": "./query/query.fp",
      "query_start": 143.488,
      "query_stop": 170.856,
      "ref_path": "song1",
      "ref_identifier": "song1",
      "ref_start": 1.272,
      "ref_stop": 28.632,
      "score": 23,
      "time_factor": 0.9998,
      "frequency_factor": 1.0,
      "percent_seconds_with_match": 0.32
    }
  ]
}
```

### 3. Opciones Avanzadas

```bash
# Monitor mode (segmentación para archivos >25s)
fpgen long_audio.mp3 ./fp/ --monitor

# Verbose logging
fpgen song.mp3 ./fp/ --verbose
fpmatcher ./db/ ./query/query.fp --verbose

# Combinar opciones
fpgen broadcast.ts ./fp/ --monitor --verbose

# Limitar resultados de matching
fpmatcher ./db/ ./query/query.fp --max-results 5
```

## 📊 Formatos Soportados

### Audio (Decoders Puros Rust)

| Formato | Extensión | Decoder | Estado |
|---------|-----------|---------|--------|
| WAV | `.wav` | `hound` | ✅ Completo |
| MP3 | `.mp3` | `minimp3` | ✅ Completo |
| FLAC | `.flac` | `claxon` | ✅ Completo |
| OGG Vorbis | `.ogg` | `lewton` | ✅ Completo |

### Video (Extracción Automática de Audio)

| Formato | Extensión | Demuxer | Estado |
|---------|-----------|---------|--------|
| MP4 | `.mp4`, `.m4a`, `.m4v` | `symphonia` (puro Rust) | ✅ Completo |
| MPEG-TS | `.ts`, `.mts`, `.m2ts` | FFmpeg pipe (puro Rust) | ✅ Completo* |
| Matroska | `.mkv` | `symphonia` (puro Rust) | ✅ Completo |
| AVI | `.avi` | `symphonia` (puro Rust) | ✅ Completo |
| QuickTime | `.mov` | `symphonia` (puro Rust) | ✅ Completo |
| WebM | `.webm` | `symphonia` (puro Rust) | ✅ Completo |

**Codecs de audio soportados en video:**
- AAC (Advanced Audio Coding)
- MP3 (MPEG-1/2 Layer 3)
- PCM (sin comprimir)
- Vorbis
- FLAC
- ADPCM

> **\*Nota sobre MPEG-TS:** Los archivos `.ts` requieren FFmpeg instalado en el sistema. El audio se extrae automáticamente vía pipe (sin archivos temporales). Si FFmpeg no está disponible, el programa mostrará instrucciones de instalación.

## ⚙️ Parámetros del Algoritmo

El algoritmo usa los mismos parámetros que Java Panako:

- **Rango de frecuencia:** 110-7040 Hz (6 octavas)
- **Bandas por octava:** 85
- **Resolución temporal:** ~8ms (128 samples @ 16kHz)
- **Ventana:** Hann
- **Filtro 2D max:** 103 bins × 25 frames
- **Hash:** 64 bits compatible con Java Panako

## 📁 Formato de Archivo `.fp`

Formato binario propio, portable y eficiente:

```
[Header: 64 bytes]
  - Magic: "FPAN"
  - Version: 1
  - Metadata size
  - Payload size
  - Num fingerprints
  - Sample rate
  - Duration (ms)
  - Channels
  - Checksum

[Metadata: Variable]
  - Algorithm ID
  - Algorithm params (JSON)
  - Original filename

[Payload: 20 bytes × num_fingerprints]
  - hash (u64)
  - t1 (i32)
  - f1 (i16)
  - padding (u16)
  - m1 (f32)
```

## 🎯 Campos de Output

### fpgen Output

| Campo | Tipo | Descripción |
|-------|------|-------------|
| `status` | string | "success" o "error" |
| `input_file` | string | Ruta del archivo procesado |
| `output_file` | string | Ruta del archivo `.fp` generado |
| `num_fingerprints` | integer | Número de fingerprints generados |
| `processing_time_seconds` | float | Tiempo de procesamiento |

### fpmatcher Output

| Campo | Tipo | Descripción |
|-------|------|-------------|
| `query_path` | string | Ruta del archivo de query |
| `detections` | integer | Número total de detecciones válidas |
| `results` | array | Array de matches (ver abajo) |

**Campos de cada match:**

| Campo | Tipo | Descripción |
|-------|------|-------------|
| `query_path` | string | Ruta del query |
| `query_start` | float | Inicio del match en query (segundos) |
| `query_stop` | float | Fin del match en query (segundos) |
| `ref_path` | string | Nombre del archivo de referencia |
| `ref_identifier` | string | Identificador de la referencia |
| `ref_start` | float | Inicio del match en referencia (segundos) |
| `ref_stop` | float | Fin del match en referencia (segundos) |
| `score` | integer | Número de fingerprints que matchearon |
| `time_factor` | float | Factor de velocidad (1.0 = normal, >1.0 = acelerado) |
| `frequency_factor` | float | Factor de pitch (1.0 = normal, >1.0 = más agudo) |
| `percent_seconds_with_match` | float | Porcentaje de segundos del query con matches (0.0-1.0) |

## 🔍 Filtrado de Matches

El sistema filtra automáticamente matches de baja calidad:

- ❌ Matches con duración < 100ms (ruido)
- ❌ Matches con cobertura < 10% (falsos positivos)
- ❌ Matches sin referencia válida

Solo se muestran matches de alta confianza.

## 📈 Performance

### fpgen
- ~3.5s para procesar archivo TS de 10MB
- ~2,241 fingerprints generados
- Sin archivos temporales

### fpmatcher
- Carga paralela de base de datos
- ~82,000 archivos/segundo en carga
- Matching sub-segundo para DBs pequeñas
- Escalable con rayon

**Ejemplo de logs:**
```
[INFO] Found 129 .fp files, loading in parallel...
[INFO] Loaded 129 files in 0.00s (82344 files/sec)
[INFO] Matching completed in 0.00s, found 3 results
```

## 🛠️ Arquitectura

```
panako-rust/
├── crates/
│   ├── panako-core/       # Algoritmo core
│   │   ├── audio/         # Decoders + resampling
│   │   ├── transform/     # FFT + Constant-Q
│   │   ├── eventpoint.rs  # Extracción de puntos
│   │   ├── fingerprint.rs # Generación de hash
│   │   └── matching.rs    # Algoritmo de matching
│   ├── panako-fp/         # Formato de archivo
│   │   ├── format.rs      # Estructuras
│   │   ├── reader.rs      # Lector binario
│   │   └── writer.rs      # Escritor binario
│   └── panako-cli/        # Binarios CLI
│       └── bin/
│           ├── fpgen.rs   # Generador
│           └── fpmatcher.rs # Matcher
└── target/release/
    ├── fpgen.exe          # 4.87 MB
    └── fpmatcher.exe      # 2.02 MB
```

## 🔮 Roadmap

### Próximas Mejoras

- [ ] **Coverage Mejorado**
  - Calcular `ref_coverage` (% de referencia matched)
  - Almacenar duración de archivos en Matcher
  - Mejor interpretación de calidad de match

- [ ] **PostgreSQL Integration**
  - Almacenar fingerprints en base de datos
  - Query desde PostgreSQL
  - Historial de matches
  - API REST opcional

- [ ] **Optimizaciones**
  - Índice `.fpi` para búsquedas más rápidas
  - Memory-mapped file reading
  - Compresión zstd opcional

- [ ] **Features Adicionales**
  - Batch processing
  - Progress bars
  - Configuración personalizable
  - Más tests de integración

Ver `implementation_plan.md` para detalles completos.

## 📚 Documentación Adicional

- **`EXAMPLES.md`** - Ejemplos de uso detallados
- **`implementation_plan.md`** - Plan técnico y roadmap
- **`task.md`** - Checklist de desarrollo
- **`walkthrough.md`** - Resumen de implementación

## 🤝 Comparación con Java Panako

| Aspecto | Java Panako | Rust Panako |
|---------|-------------|-------------|
| **Runtime** | JRE + FFmpeg | Ninguno (FFmpeg solo para TS) |
| **Tamaño binario** | ~50MB (JAR + deps) | ~5MB total |
| **Storage** | LMDB database | Archivos `.fp` portables |
| **Output** | CSV/texto | JSON estructurado |
| **Portabilidad** | Requiere instalación | Copy & run |
| **Velocidad** | ~80s audio/s | ~80s audio/s |
| **Hash algorithm** | ✅ Compatible | ✅ Compatible |
| **Determinismo** | ✅ Sí | ✅ Sí |

## 📄 Licencia

AGPL-3.0 (igual que Java Panako)

## 🙏 Créditos

Port a Rust del proyecto original [Panako](https://github.com/JorenSix/Panako) por Joren Six.
