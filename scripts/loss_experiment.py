import json
import shutil
import subprocess
import tempfile
from datetime import datetime
from multiprocessing import Pool
from pathlib import Path

import librosa
import matplotlib.pyplot as plt
import numpy as np


def convert_wav_to_opus(wav_path, opus_path, bitrate="128k"):
    subprocess.run(
        [
            "ffmpeg",
            "-i",
            str(wav_path),
            "-c:a",
            "libopus",
            "-b:a",
            bitrate,
            "-y",
            str(opus_path),
        ],
        check=True,
        capture_output=True,
    )


def convert_opus_to_wav(opus_path, wav_path):
    subprocess.run(
        ["ffmpeg", "-i", str(opus_path), "-acodec", "pcm_s16le", "-y", str(wav_path)],
        check=True,
        capture_output=True,
    )


def get_audio_metrics(orig_signal, processed_signal):
    min_len = min(len(orig_signal), len(processed_signal))
    orig = orig_signal[:min_len]
    proc = processed_signal[:min_len]

    mse = np.mean((orig - proc) ** 2)
    psnr = 10 * np.log10(1.0 / mse) if mse > 0 else float("inf")
    rms_diff = abs(np.sqrt(np.mean(orig**2)) - np.sqrt(np.mean(proc**2)))

    return {"psnr": psnr, "mse": mse, "rms_diff": rms_diff}


def calculate_metrics_parallel(args):
    orig_signal, wav_path, sr = args
    processed_signal, _ = librosa.load(str(wav_path), sr=sr, mono=True)
    return get_audio_metrics(orig_signal, processed_signal)


input_path = Path.home() / "Downloads" / "samples_gb0.wav"
iterations = 10_000
num_processes = 4

temp_dir = Path(tempfile.mkdtemp())
output_dir = input_path.parent / f"output_{datetime.now().strftime('%Y%m%d_%H%M%S')}"
checkpoint_dir = output_dir / "checkpoints"
output_dir.mkdir(parents=True)
checkpoint_dir.mkdir()

# Load original signal once
orig_signal, sr = librosa.load(str(input_path), sr=None, mono=True)

# Sequential processing
current = input_path
wav_files = []  # Keep track of WAV files for parallel metrics calculation

print(f"Starting {iterations} iterations...")
for i in range(iterations):
    opus = temp_dir / f"temp_{i}.opus"
    wav = temp_dir / f"temp_{i}.wav"

    convert_wav_to_opus(current, opus)
    convert_opus_to_wav(opus, wav)

    # Save checkpoint every 100 iterations
    if i % 100 == 0:
        checkpoint = checkpoint_dir / f"checkpoint_{i}.wav"
        shutil.copy2(wav, checkpoint)
        print(f"Completed {i} iterations...")

    wav_files.append(wav)
    current = wav
    opus.unlink()  # Clean up opus file after use

    if i > 0:  # Keep the previous WAV file until metrics are calculated
        wav_files[-2].unlink()

# Calculate metrics in parallel
print("Processing complete. Calculating metrics...")
metrics_args = [(orig_signal, wav, sr) for wav in wav_files]
with Pool(num_processes) as pool:
    metrics = pool.map(calculate_metrics_parallel, metrics_args)

# Save final result
final_output = output_dir / f"final_{iterations}_iterations.wav"
shutil.copy2(current, final_output)

# Save metrics
metrics_file = output_dir / "metrics.json"
with open(metrics_file, "w") as f:
    json.dump(metrics, f)

# Plot metrics
iters = range(1, len(metrics) + 1)
plt.figure(figsize=(12, 8))
plt.plot(iters, [m["psnr"] for m in metrics], label="PSNR")
plt.plot(iters, [m["rms_diff"] for m in metrics], label="RMS Diff")
plt.xlabel("Iteration")
plt.ylabel("Value")
plt.title("Audio Quality Metrics Over Iterations")
plt.legend()
plt.grid(True)

plot_path = output_dir / f"metrics_{datetime.now().strftime('%Y%m%d_%H%M%S')}.png"
plt.savefig(plot_path)
plt.close()

# Cleanup
shutil.rmtree(temp_dir)
print("Processing complete!")
