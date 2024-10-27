import json
import shutil
import subprocess
import tempfile
import time
from datetime import datetime, timedelta
from pathlib import Path

import librosa
import matplotlib.pyplot as plt
import numpy as np
import seaborn as sns
from scipy.signal import welch
from scipy.stats import entropy


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


def calculate_noise_entropy(signal, frame_length=2048, hop_length=512):
    """Measure how 'noisy' vs 'structured' the signal is."""
    # Compute STFT
    D = librosa.stft(signal, n_fft=frame_length, hop_length=hop_length)
    S = np.abs(D)
    
    # Spectral flatness (high = noise-like, low = tonal)
    spectral_flatness = np.exp(np.mean(np.log(S + 1e-10), axis=0)) / (np.mean(S, axis=0) + 1e-10)
    
    # Spectral entropy across frequency bins
    S_norm = S / (np.sum(S, axis=0) + 1e-10)
    spectral_entropy = np.mean([entropy(frame) for frame in S_norm.T])
    
    # Std of local segments
    segments = librosa.util.frame(signal, frame_length=1024, hop_length=512)
    temporal_variation = np.std(np.std(segments, axis=0))

    return {
        'spectral_flatness': float(np.mean(spectral_flatness)),
        'spectral_entropy': float(spectral_entropy),
        'temporal_variation': float(temporal_variation)
    }


def calculate_audio_metrics(orig_signal, processed_signal):
    min_len = min(len(orig_signal), len(processed_signal))
    orig = orig_signal[:min_len]
    proc = processed_signal[:min_len]

    # Basic metrics
    mse = np.mean((orig - proc) ** 2)
    psnr = 10 * np.log10(1.0 / mse) if mse > 0 else float("inf")
    rms_diff = abs(np.sqrt(np.mean(orig**2)) - np.sqrt(np.mean(proc**2)))

    # Spectral analysis
    freqs1, psd1 = welch(orig)
    freqs2, psd2 = welch(proc)
    spectral_diff = np.mean(np.abs(psd1 - psd2))

    # Add noise/entropy metrics
    noise_metrics = calculate_noise_entropy(proc)

    return {
        "psnr": float(psnr),
        "mse": float(mse),
        "rms_diff": float(rms_diff),
        "spectral_diff": float(spectral_diff),
        "noise_metrics": noise_metrics,
    }


def plot_progress(
    metrics,
    plot_path,
    include_spectrograms=False,
    orig_signal=None,
    current_signal=None,
):
    fig = plt.figure(figsize=(15, 15))
    gs = plt.GridSpec(3, 3, figure=fig)

    iters = range(1, len(metrics) + 1)

    # PSNR
    ax1 = fig.add_subplot(gs[0, 0])
    ax1.plot(iters, [m["psnr"] for m in metrics])
    ax1.set_title("PSNR")
    ax1.set_xlabel("Iterations")
    ax1.set_ylabel("dB")
    ax1.grid(True)

    # Spectral flatness
    ax2 = fig.add_subplot(gs[0, 1])
    ax2.plot(iters, [m["noise_metrics"]["spectral_flatness"] for m in metrics])
    ax2.set_title("Spectral Flatness\n(higher = more noise-like)")
    ax2.set_xlabel("Iterations")
    ax2.set_ylabel("Ratio")
    ax2.grid(True)

    # Spectral entropy
    ax3 = fig.add_subplot(gs[0, 2])
    ax3.plot(iters, [m["noise_metrics"]["spectral_entropy"] for m in metrics])
    ax3.set_title("Spectral Entropy\n(higher = more random)")
    ax3.set_xlabel("Iterations")
    ax3.set_ylabel("Bits")
    ax3.grid(True)

    # Temporal predictability
    ax4 = fig.add_subplot(gs[1, 0])
    ax4.plot(iters, [m["noise_metrics"]["temporal_variation"] for m in metrics])
    ax4.set_title("Temporal Predictability\n(lower = more random)")
    ax4.set_xlabel("Iterations")
    ax4.set_ylabel("Correlation")
    ax4.grid(True)

    # RMS difference
    ax5 = fig.add_subplot(gs[1, 1])
    ax5.plot(iters, [m["rms_diff"] for m in metrics])
    ax5.set_title("RMS Amplitude Difference")
    ax5.set_xlabel("Iterations")
    ax5.set_ylabel("Difference")
    ax5.grid(True)

    # MSE
    ax6 = fig.add_subplot(gs[1, 2])
    ax6.plot(iters, [m["mse"] for m in metrics])
    ax6.set_title("Mean Squared Error")
    ax6.set_xlabel("Iterations")
    ax6.set_ylabel("Error")
    ax6.set_yscale("log")
    ax6.grid(True)

    # Spectral difference
    ax7 = fig.add_subplot(gs[2, 0])
    ax7.plot(iters, [m["spectral_diff"] for m in metrics])
    ax7.set_title("Spectral Power Difference")
    ax7.set_xlabel("Iterations")
    ax7.set_ylabel("Difference")
    ax7.grid(True)

    if include_spectrograms and orig_signal is not None and current_signal is not None:
        # Save spectrograms to a separate file
        fig_spec, (ax1, ax2, ax3) = plt.subplots(1, 3, figsize=(15, 5))

        # Original spectrogram
        D_orig = librosa.amplitude_to_db(np.abs(librosa.stft(orig_signal)), ref=np.max)
        librosa.display.specshow(D_orig, y_axis="log", x_axis="time", ax=ax1)
        ax1.set_title("Original Spectrogram")

        # Current spectrogram
        D_curr = librosa.amplitude_to_db(
            np.abs(librosa.stft(current_signal)), ref=np.max
        )
        librosa.display.specshow(D_curr, y_axis="log", x_axis="time", ax=ax2)
        ax2.set_title(f"Iteration {len(metrics)} Spectrogram")

        # Difference spectrogram
        diff = D_orig - D_curr
        im = librosa.display.specshow(diff, y_axis="log", x_axis="time", ax=ax3)
        plt.colorbar(im, ax=ax3)
        ax3.set_title("Difference (dB)")

        spec_path = plot_path.parent / f"spectrograms_{len(metrics)}.png"
        plt.savefig(spec_path, dpi=300, bbox_inches="tight")
        plt.close(fig_spec)

    plt.tight_layout()
    plt.savefig(plot_path, dpi=300, bbox_inches="tight")
    plt.close()


def format_time(seconds):
    return str(timedelta(seconds=int(seconds)))


def main(input_file, iterations=10000, save_interval=100, metrics_interval=10):
    print(f"Starting audio compression experiment with {iterations} iterations...")

    input_path = Path(input_file)
    temp_dir = Path(tempfile.mkdtemp())
    output_dir = (
        input_path.parent / f"output_{datetime.now().strftime('%Y%m%d_%H%M%S')}"
    )
    checkpoint_dir = output_dir / "checkpoints"
    output_dir.mkdir(parents=True)
    checkpoint_dir.mkdir()

    print(f"Loading original signal from {input_path}...")
    start_load = time.time()
    orig_signal, sr = librosa.load(str(input_path), sr=None, mono=True)
    print(f"Signal loaded in {time.time() - start_load:.1f}s. Starting iterations...")

    current = input_path
    metrics = []
    start_time = time.time()

    for i in range(iterations):
        opus = temp_dir / f"temp_{i}.opus"
        wav = temp_dir / f"temp_{i}.wav"

        convert_wav_to_opus(current, opus)
        convert_opus_to_wav(opus, wav)

        # Calculate metrics more frequently than saving
        if i % metrics_interval == 0:
            processed_signal, _ = librosa.load(str(wav), sr=sr, mono=True)
            current_metrics = calculate_audio_metrics(orig_signal, processed_signal)
            metrics.append(current_metrics)

            # Estimate time remaining
            elapsed = time.time() - start_time
            rate = (i + 1) / elapsed
            remaining = (iterations - i - 1) / rate

            # Print progress with metrics
            print(
                f"[{i+1}/{iterations}] "
                f"PSNR: {current_metrics['psnr']:.2f}dB | "
                f"Spectral Flatness: {current_metrics['noise_metrics']['spectral_flatness']:.3f} | "
                f"Rate: {rate:.1f} it/s | "
                f"Elapsed: {format_time(elapsed)} | "
                f"Remaining: {format_time(remaining)}"
            )

        # Save checkpoints and update plots at save_interval
        if i % save_interval == 0:
            checkpoint = checkpoint_dir / f"checkpoint_{i}.wav"
            shutil.copy2(wav, checkpoint)

            # Save metrics and plot
            metrics_file = output_dir / "metrics.json"
            with open(metrics_file, "w") as f:
                json.dump(
                    {
                        "last_iteration": i,
                        "metrics": metrics,
                        "timestamp": datetime.now().isoformat(),
                    },
                    f,
                )

            plot_progress(
                metrics,
                output_dir / "metrics.png",
                include_spectrograms=(i % 1000 == 0),
                orig_signal=orig_signal,
                current_signal=processed_signal,
            )

            last_save_time = time.time()

        current = wav
        opus.unlink()

        if i > 0:  # Keep the previous WAV file until metrics are calculated
            (temp_dir / f"temp_{i-1}.wav").unlink()

    # Save final result
    final_output = output_dir / f"final_{iterations}_iterations.wav"
    shutil.copy2(current, final_output)

    # Cleanup
    shutil.rmtree(temp_dir)
    print("Processing complete!")


if __name__ == "__main__":
    input_file = str(Path.home() / "Downloads" / "samples_gb0.wav")
    main(input_file, iterations=10000)
