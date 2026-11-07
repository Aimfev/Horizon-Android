package dev.horizon.android;

/** SAF result callback only. The native activity, UI and file operations stay Rust. */
public final class HorizonActivity extends android.app.NativeActivity {
    static { System.loadLibrary("horizon_android"); }
    private native void nativeDocumentResult(int request, int result, String uri);

    @Override protected void onActivityResult(int request, int result, android.content.Intent data) {
        super.onActivityResult(request, result, data);
        if (request != 4101 && request != 4102) return;
        nativeDocumentResult(request, result,
                data == null || data.getData() == null ? null : data.getData().toString());
    }

    public static String copyFolderTree(android.content.Context context, String treeUriString, String destDirPath) throws Exception {
        android.net.Uri treeUri = android.net.Uri.parse(treeUriString);
        String treeDocId = android.provider.DocumentsContract.getTreeDocumentId(treeUri);
        android.net.Uri docUri = android.provider.DocumentsContract.buildDocumentUriUsingTree(treeUri, treeDocId);

        String folderName = "imported_folder";
        try (android.database.Cursor c = context.getContentResolver().query(
                docUri, new String[]{android.provider.DocumentsContract.Document.COLUMN_DISPLAY_NAME}, null, null, null)) {
            if (c != null && c.moveToFirst()) {
                String name = c.getString(0);
                if (name != null && !name.trim().isEmpty()) {
                    folderName = name.trim();
                }
            }
        }
        folderName = folderName.replaceAll("[/\\\\:*?\"<>|]", "_");
        java.io.File destFolder = new java.io.File(destDirPath, folderName);
        int suffix = 1;
        while (destFolder.exists()) {
            destFolder = new java.io.File(destDirPath, folderName + " (" + suffix + ")");
            suffix++;
        }
        if (!destFolder.mkdirs() && !destFolder.exists()) {
            throw new java.io.IOException("Failed to create folder: " + destFolder.getAbsolutePath());
        }
        copyFolderChildren(context.getContentResolver(), treeUri, docUri, destFolder);
        return destFolder.getName();
    }

    private static void copyFolderChildren(android.content.ContentResolver resolver,
            android.net.Uri treeUri, android.net.Uri parentDocUri, java.io.File destDir) throws Exception {
        String docId = android.provider.DocumentsContract.getDocumentId(parentDocUri);
        android.net.Uri childrenUri = android.provider.DocumentsContract.buildChildDocumentsUriUsingTree(treeUri, docId);
        String[] projection = new String[] {
            android.provider.DocumentsContract.Document.COLUMN_DOCUMENT_ID,
            android.provider.DocumentsContract.Document.COLUMN_DISPLAY_NAME,
            android.provider.DocumentsContract.Document.COLUMN_MIME_TYPE
        };
        try (android.database.Cursor c = resolver.query(childrenUri, projection, null, null, null)) {
            if (c == null) return;
            while (c.moveToNext()) {
                String childDocId = c.getString(0);
                String childName = c.getString(1);
                String mimeType = c.getString(2);
                if (childName == null || childName.isEmpty()) continue;
                childName = childName.replaceAll("[/\\\\:*?\"<>|]", "_");
                android.net.Uri childUri = android.provider.DocumentsContract.buildDocumentUriUsingTree(treeUri, childDocId);
                if (android.provider.DocumentsContract.Document.MIME_TYPE_DIR.equals(mimeType)) {
                    java.io.File subDir = new java.io.File(destDir, childName);
                    if (!subDir.exists()) subDir.mkdirs();
                    copyFolderChildren(resolver, treeUri, childUri, subDir);
                } else {
                    java.io.File targetFile = new java.io.File(destDir, childName);
                    try (java.io.InputStream in = resolver.openInputStream(childUri);
                         java.io.FileOutputStream out = new java.io.FileOutputStream(targetFile)) {
                        if (in != null) {
                            byte[] buffer = new byte[65536];
                            int read;
                            while ((read = in.read(buffer)) != -1) {
                                out.write(buffer, 0, read);
                            }
                            out.flush();
                        }
                    }
                }
            }
        }
    }

    public static void installApk(android.content.Context context, String apkFilePath) {
        java.io.File file = new java.io.File(apkFilePath);
        if (!file.exists()) return;
        try {
            java.lang.reflect.Method m = android.os.StrictMode.class.getMethod("disableDeathOnFileUriExposure");
            m.invoke(null);
        } catch (Throwable ignored) {}
        android.content.Intent intent = new android.content.Intent(android.content.Intent.ACTION_VIEW);
        intent.addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK);
        intent.addFlags(android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION);
        intent.setDataAndType(android.net.Uri.fromFile(file), "application/vnd.android.package-archive");
        try {
            context.startActivity(intent);
        } catch (Throwable error) {
            android.util.Log.e("HorizonActivity", "Failed to launch package installer", error);
        }
    }
}
