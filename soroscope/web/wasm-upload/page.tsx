import WasmUpload from "../components/WasmUpload";

export default function WasmUploadPage() {
  return (
    <main className="min-h-screen bg-slate-50 py-12 px-4">
      <div className="max-w-3xl mx-auto">
        <div className="text-center mb-8">
          <h1 className="text-3xl font-bold text-slate-900 mb-2">
            Soroban Contract Upload
          </h1>
          <p className="text-slate-600">
            Upload your compiled WASM files to analyze resource consumption
          </p>
        </div>

        <WasmUpload
          maxFiles={3}
          maxFileSize={5 * 1024 * 1024}
          onFileSelect={(files) => {
            console.log("Files selected:", files.map((f) => f.name));
          }}
          onUploadComplete={(files) => {
            console.log(
              "Ready for analysis:",
              files.map((f) => ({
                name: f.file.name,
                hash: f.hash,
                size: f.file.size,
              }))
            );
            // Here you'd navigate to the analysis dashboard or send to the core API
          }}
        />
      </div>
    </main>
  );
}