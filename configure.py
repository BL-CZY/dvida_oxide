import argparse
import subprocess
import os

parser = argparse.ArgumentParser()
parser.add_argument("--arch")

args = parser.parse_args()

subprocess.Popen(["cp", f"makefiles/Makefile_{args.arch}","GNUMakefile"])

os.chdir("./kernel/")

subprocess.Popen(["cp", f"makefiles/Makefile_{args.arch}","GNUMakefile"])
