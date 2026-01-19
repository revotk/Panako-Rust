# Panako Rust - Ejemplos de Uso

Ejemplos prácticos de uso de `fpgen` y `fpmatcher`.

---

## 📋 Tabla de Contenidos

1. [Generación de Fingerprints](#generación-de-fingerprints)
2. [Matching Básico](#matching-básico)
3. [Casos de Uso Reales](#casos-de-uso-reales)
4. [Integración con Scripts](#integración-con-scripts)
5. [Interpretación de Resultados](#interpretación-de-resultados)

---

## Generación de Fingerprints

### Audio Simple

```bash
# WAV
fpgen audio.wav ./fingerprints/

# MP3
fpgen song.mp3 ./fingerprints/

# FLAC
fpgen album.flac ./fingerprints/

# OGG
fpgen podcast.ogg ./fingerprints/
```

**Output esperado:**
```json
{
  "status": "success",
  "input_file": "song.mp3",
  "output_file": "./fingerprints/song.fp",
  "duration_seconds": 180.5,
  "num_fingerprints": 1523,
  "processing_time_seconds": 0.45
}
```

### Video (Extracción Automática)

```bash
# MP4
fpgen video.mp4 ./fingerprints/

# MKV
fpgen movie.mkv ./fingerprints/

# AVI
fpgen clip.avi ./fingerprints/

# MOV
fpgen recording.mov ./fingerprints/

# MPEG-TS (requiere FFmpeg)
fpgen broadcast.ts ./fingerprints/
```

**Output esperado:**
```json
{
  "status": "success",
  "input_file": "video.mp4",
  "output_file": "./fingerprints/video.fp",
  "num_fingerprints": 2241,
  "processing_time_seconds": 3.58
}
```

### Procesamiento por Lotes

**Windows (PowerShell):**
```powershell
# Procesar todos los MP3 en un directorio
Get-ChildItem .\audio\*.mp3 | ForEach-Object {
    .\fpgen.exe $_.FullName .\fingerprints\
}

# Con logging
Get-ChildItem .\audio\*.mp3 | ForEach-Object {
    Write-Host "Processing: $($_.Name)"
    .\fpgen.exe $_.FullName .\fingerprints\ --verbose
}
```

**Linux/macOS (Bash):**
```bash
# Procesar todos los archivos de audio
for file in ./audio/*; do
    fpgen "$file" ./fingerprints/
done

# Solo MP3 y FLAC
for file in ./audio/*.{mp3,flac}; do
    [ -f "$file" ] && fpgen "$file" ./fingerprints/
done
```

---

## Matching Básico

### Caso 1: Sin Resultados

```bash
fpmatcher ./db/ ./query/unknown.fp
```

**Output:**
```json
{
  "query_path": "./query/unknown.fp",
  "detections": 0,
  "results": []
}
```

**Interpretación:** No se encontraron matches válidos.

### Caso 2: Match Perfecto

```bash
fpmatcher ./db/ ./query/song1.fp
```

**Output:**
```json
{
  "query_path": "./query/song1.fp",
  "detections": 1,
  "results": [
    {
      "query_path": "./query/song1.fp",
      "query_start": 0.0,
      "query_stop": 180.5,
      "ref_path": "song1",
      "ref_identifier": "song1",
      "ref_start": 0.0,
      "ref_stop": 180.5,
      "score": 1847,
      "time_factor": 1.0,
      "frequency_factor": 1.0,
      "percent_seconds_with_match": 0.98
}
  ]
}
```

**Interpretación:**
- Match completo (98% de cobertura)
- Velocidad normal (time_factor = 1.0)
- Pitch normal (frequency_factor = 1.0)
- Alta confianza (score = 1847)

### Caso 3: Múltiples Detecciones

```bash
fpmatcher ./db/ ./query/mix.fp
```

**Output:**
```json
{
  "query_path": "./query/mix.fp",
  "detections": 3,
  "results": [
    {
      "query_path": "./query/mix.fp",
      "query_start": 143.488,
      "query_stop": 170.856,
      "ref_path": "song_a",
      "ref_identifier": "song_a",
      "ref_start": 1.272,
      "ref_stop": 28.632,
      "score": 23,
      "time_factor": 0.9998,
      "frequency_factor": 1.0,
      "percent_seconds_with_match": 0.32
    },
    {
      "query_path": "./query/mix.fp",
      "query_start": 82.944,
      "query_stop": 109.608,
      "ref_path": "song_b",
      "ref_identifier": "song_b",
      "ref_start": 1.008,
      "ref_stop": 27.68,
      "score": 15,
      "time_factor": 0.9999,
      "frequency_factor": 1.0,
      "percent_seconds_with_match": 0.37
    },
    {
      "query_path": "./query/mix.fp",
      "query_start": 180.152,
      "query_stop": 198.04,
      "ref_path": "song_c",
      "ref_identifier": "song_c",
      "ref_start": 7.8,
      "ref_stop": 25.696,
      "score": 10,
      "time_factor": 0.9999,
      "frequency_factor": 1.0,
      "percent_seconds_with_match": 0.39
    }
  ]
}
```

**Interpretación:**
- 3 canciones diferentes detectadas
- Cada una en diferentes momentos del query
- Cobertura moderada (30-40%)

---

## Casos de Uso Reales

### 1. Detección de Copyright en Broadcast

**Escenario:** Monitorear transmisiones de radio/TV para detectar uso de música.

```bash
# Paso 1: Crear DB de canciones protegidas
for song in ./protected_music/*.mp3; do
    fpgen "$song" ./copyright_db/
done

# Paso 2: Procesar grabación de broadcast
fpgen broadcast_recording.ts ./queries/

# Paso 3: Buscar matches
fpmatcher ./copyright_db/ ./queries/broadcast_recording.fp --max-results 100
```

### 2. Identificación de Samples en Producción Musical

**Escenario:** Detectar qué samples se usaron en una producción.

```bash
# DB de samples
fpgen sample_pack/*.wav ./sample_db/

# Analizar producción
fpgen my_track.mp3 ./query/

# Buscar samples usados
fpmatcher ./sample_db/ ./query/my_track.fp
```

### 3. Deduplicación de Biblioteca Musical

**Escenario:** Encontrar duplicados en una colección grande.

```bash
# Generar fingerprints de toda la biblioteca
for file in ./music_library/**/*.{mp3,flac}; do
    fpgen "$file" ./library_fp/
done

# Para cada archivo, buscar duplicados
for fp in ./library_fp/*.fp; do
    echo "Checking: $fp"
    fpmatcher ./library_fp/ "$fp" --max-results 5
done
```

### 4. Verificación de Calidad de Streaming

**Escenario:** Verificar que el audio transmitido coincide con el original.

```bash
# Original
fpgen original.wav ./reference/

# Grabación del stream
fpgen stream_recording.mp3 ./test/

# Comparar
fpmatcher ./reference/ ./test/stream_recording.fp
```

**Verificar:**
- `time_factor` cercano a 1.0 (sin aceleración)
- `frequency_factor` cercano a 1.0 (sin pitch shift)
- `percent_seconds_with_match` > 0.9 (alta cobertura)

---

## Integración con Scripts

### Python

```python
import subprocess
import json

def generate_fingerprint(audio_file, output_dir):
    """Genera fingerprint y retorna metadata"""
    result = subprocess.run(
        ['fpgen', audio_file, output_dir],
        capture_output=True,
        text=True
    )
    return json.loads(result.stdout)

def find_matches(db_dir, query_fp, max_results=10):
    """Busca matches y retorna resultados"""
    result = subprocess.run(
        ['fpmatcher', db_dir, query_fp, '--max-results', str(max_results)],
        capture_output=True,
        text=True
    )
    return json.loads(result.stdout)

# Uso
fp_info = generate_fingerprint('song.mp3', './fp/')
print(f"Generated {fp_info['num_fingerprints']} fingerprints")

matches = find_matches('./db/', './fp/song.fp')
print(f"Found {matches['detections']} matches")

for match in matches['results']:
    print(f"  - {match['ref_identifier']}: score={match['score']}")
```

### Node.js

```javascript
const { execSync } = require('child_process');

function generateFingerprint(audioFile, outputDir) {
    const output = execSync(`fpgen "${audioFile}" "${outputDir}"`);
    return JSON.parse(output.toString());
}

function findMatches(dbDir, queryFp, maxResults = 10) {
    const output = execSync(
        `fpmatcher "${dbDir}" "${queryFp}" --max-results ${maxResults}`
    );
    return JSON.parse(output.toString());
}

// Uso
const fpInfo = generateFingerprint('song.mp3', './fp/');
console.log(`Generated ${fpInfo.num_fingerprints} fingerprints`);

const matches = findMatches('./db/', './fp/song.fp');
console.log(`Found ${matches.detections} matches`);

matches.results.forEach(match => {
    console.log(`  - ${match.ref_identifier}: score=${match.score}`);
});
```

### Bash Script Completo

```bash
#!/bin/bash

DB_DIR="./fingerprint_db"
QUERY_DIR="./queries"
RESULTS_DIR="./results"

# Crear directorios
mkdir -p "$DB_DIR" "$QUERY_DIR" "$RESULTS_DIR"

# Función para procesar archivo
process_file() {
    local file="$1"
    local basename=$(basename "$file" | sed 's/\.[^.]*$//')
    
    echo "Processing: $file"
    
    # Generar fingerprint
    fpgen "$file" "$QUERY_DIR" > "$RESULTS_DIR/${basename}_fp.json"
    
    # Buscar matches
    fpmatcher "$DB_DIR" "$QUERY_DIR/${basename}.fp" \
        > "$RESULTS_DIR/${basename}_matches.json"
    
    # Mostrar resumen
    local detections=$(jq -r '.detections' "$RESULTS_DIR/${basename}_matches.json")
    echo "  → Found $detections matches"
}

# Procesar todos los archivos
for file in ./audio/*.mp3; do
    process_file "$file"
done

echo "Done! Results in $RESULTS_DIR/"
```

---

## Interpretación de Resultados

### Campos Clave

#### `score`
- **Qué es:** Número de fingerprints que matchearon
- **Interpretación:**
  - < 10: Match débil, posible falso positivo
  - 10-50: Match moderado
  - > 50: Match fuerte, alta confianza

#### `time_factor`
- **Qué es:** Ratio de velocidad (query vs referencia)
- **Interpretación:**
  - 1.0: Velocidad normal
  - 1.1: 10% más rápido
  - 0.9: 10% más lento
  - Fuera de 0.95-1.05: Posible time-stretching

#### `frequency_factor`
- **Qué es:** Ratio de pitch (query vs referencia)
- **Interpretación:**
  - 1.0: Pitch normal
  - 1.06: ~1 semitono más alto
  - 0.94: ~1 semitono más bajo
  - Fuera de 0.98-1.02: Posible pitch-shifting

#### `percent_seconds_with_match`
- **Qué es:** Porcentaje de segundos del query con matches
- **Interpretación:**
  - > 0.8: Cobertura excelente
  - 0.5-0.8: Cobertura buena
  - 0.2-0.5: Cobertura moderada
  - < 0.2: Cobertura baja, posible fragmento

### Ejemplos de Interpretación

#### Match Perfecto
```json
{
  "score": 1847,
  "time_factor": 1.0,
  "frequency_factor": 1.0,
  "percent_seconds_with_match": 0.98
}
```
→ **Conclusión:** Copia exacta, sin modificaciones

#### Audio Acelerado
```json
{
  "score": 823,
  "time_factor": 1.15,
  "frequency_factor": 1.0,
  "percent_seconds_with_match": 0.85
}
```
→ **Conclusión:** Audio 15% más rápido (posible time-stretch sin pitch change)

#### Sample/Fragmento
```json
{
  "score": 45,
  "time_factor": 1.0,
  "frequency_factor": 1.0,
  "percent_seconds_with_match": 0.25
}
```
→ **Conclusión:** Solo 25% del query matchea, probablemente un sample o fragmento

#### Pitch Shift
```json
{
  "score": 234,
  "time_factor": 1.0,
  "frequency_factor": 1.12,
  "percent_seconds_with_match": 0.67
}
```
→ **Conclusión:** Pitch ~2 semitonos más alto, posible remix o versión alternativa

---

## Filtrado Automático

El sistema filtra automáticamente:

- ❌ Matches con duración < 100ms
- ❌ Matches con coverage < 10%
- ❌ Matches sin referencia válida

**Ejemplo de match filtrado:**
```json
{
  "query_start": 232.912,
  "query_stop": 232.912,  // ← Duración = 0
  "score": 10,
  "percent_seconds_with_match": 0.0  // ← Coverage = 0%
}
```
→ Este match NO aparecerá en los resultados

---

## Tips y Mejores Prácticas

### 1. Organización de Archivos

```
project/
├── audio/              # Archivos originales
├── fingerprints/       # Base de datos de .fp
├── queries/            # Queries temporales
└── results/            # Resultados JSON
```

### 2. Naming Convention

```bash
# Usar nombres descriptivos
fpgen "Artist - Song Title.mp3" ./fp/
# Genera: ./fp/Artist - Song Title.fp

# Evitar caracteres especiales
# ✅ good: song_name.mp3
# ❌ bad: song@#$%.mp3
```

### 3. Performance

```bash
# Para DBs grandes, usar --max-results
fpmatcher ./large_db/ ./query.fp --max-results 20

# Usar --verbose solo para debugging
fpmatcher ./db/ ./query.fp --verbose
```

### 4. Validación de Resultados

```python
def is_high_quality_match(match):
    """Valida si un match es de alta calidad"""
    return (
        match['score'] > 20 and
        match['percent_seconds_with_match'] > 0.3 and
        abs(match['time_factor'] - 1.0) < 0.1 and
        abs(match['frequency_factor'] - 1.0) < 0.1
    )

# Filtrar solo matches de calidad
quality_matches = [m for m in results['results'] if is_high_quality_match(m)]
```

---

## Troubleshooting

### Problema: "FFmpeg not found" con archivos .ts

**Solución:**
```bash
# Windows
# Descargar FFmpeg de https://ffmpeg.org/download.html
# Agregar a PATH

# Linux
sudo apt install ffmpeg

# macOS
brew install ffmpeg
```

### Problema: No se encuentran matches esperados

**Verificar:**
1. ¿El archivo está en la base de datos?
2. ¿La calidad del audio es suficiente?
3. ¿Hay modificaciones significativas (pitch, speed)?

```bash
# Verificar que el archivo esté en DB
ls ./db/*.fp | grep "expected_song"

# Probar con verbose
fpmatcher ./db/ ./query.fp --verbose
```

### Problema: Demasiados falsos positivos

**Solución:** Aumentar threshold en código o filtrar resultados:

```python
# Filtrar por score mínimo
valid_matches = [m for m in results['results'] if m['score'] > 50]
```

---

Para más información, ver `README.md` y `implementation_plan.md`.
