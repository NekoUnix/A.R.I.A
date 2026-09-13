fn main() -> anyhow::Result<()> {
    aria_live2d::host::serve(std::io::stdin().lock(), std::io::stdout().lock())
}
