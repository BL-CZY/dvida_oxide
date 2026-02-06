import argparse
import subprocess
import os

parser = argparse.ArgumentParser()
parser.add_argument("--arch")

args = parser.parse_args()

subprocess.Popen(["cp", "-f", f"makefiles/Makefile_{args.arch}","GNUmakefile"])

os.chdir("./kernel/")

subprocess.Popen(["cp", "-f", f"arch_specific_configs/config.{args.arch}.toml", ".cargo/config.toml"])
