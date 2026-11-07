package dev.horizon.android;

/** SAF result callback only. The native activity, UI and file operations stay Rust. */
public final class HorizonActivity extends android.app.NativeActivity {
    static { System.loadLibrary("horizon_android"); }
    private native void nativeDocumentResult(int request, int result, String uri);

    @Override protected void onCreate(android.os.Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        applyHighRefreshRate(this, 0);
    }

    @Override protected void onResume() {
        super.onResume();
        applyHighRefreshRate(this, 0);
    }

    @Override public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        if (hasFocus) {
            applyHighRefreshRate(this, 0);
        }
    }

    public static void applyHighRefreshRate(final android.app.Activity activity, final int preferredHz) {
        if (activity == null) return;
        activity.runOnUiThread(new Runnable() {
            @Override public void run() {
                try {
                    android.view.Window window = activity.getWindow();
                    if (window == null) return;
                    android.view.Display display = window.getWindowManager().getDefaultDisplay();
                    if (display == null) return;
                    android.view.Display.Mode current = display.getMode();
                    android.view.Display.Mode[] modes = display.getSupportedModes();
                    android.view.Display.Mode best = current;
                    for (android.view.Display.Mode m : modes) {
                        if (current != null && (m.getPhysicalWidth() != current.getPhysicalWidth()
                                || m.getPhysicalHeight() != current.getPhysicalHeight())) {
                            continue;
                        }
                        if (preferredHz > 0 && Math.abs(m.getRefreshRate() - preferredHz) < 1.5f) {
                            best = m;
                            break;
                        }
                        if (best == null || m.getRefreshRate() > best.getRefreshRate()) {
                            best = m;
                        }
                    }
                    android.view.WindowManager.LayoutParams lp = window.getAttributes();
                    if (best != null) {
                        lp.preferredDisplayModeId = best.getModeId();
                        lp.preferredRefreshRate = best.getRefreshRate();
                    } else if (preferredHz > 0) {
                        lp.preferredRefreshRate = (float) preferredHz;
                    }
                    window.setAttributes(lp);
                } catch (Throwable ignored) {}
            }
        });
    }

    @Override protected void onActivityResult(int request, int result, android.content.Intent data) {
        super.onActivityResult(request, result, data);
        if (request != 4101 && request != 4102 && request != 4103) return;
        android.net.Uri uri = (data == null) ? null : data.getData();
        if (result == -1 && uri != null && request == 4102) {
            try {
                final int takeFlags = data.getFlags()
                        & (android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION
                         | android.content.Intent.FLAG_GRANT_WRITE_URI_PERMISSION);
                getContentResolver().takePersistableUriPermission(
                        uri,
                        takeFlags != 0 ? takeFlags : android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION);
            } catch (Throwable ignored) {}
        }
        nativeDocumentResult(request, result, uri == null ? null : uri.toString());
    }

    public static String copyFolderTree(android.content.Context context, String treeUriString, String destDirPath) {
        try {
            if (context == null || treeUriString == null || destDirPath == null) {
                return "ERROR: Invalid folder import arguments";
            }
            android.net.Uri treeUri = android.net.Uri.parse(treeUriString);
            String treeDocId;
            try {
                treeDocId = android.provider.DocumentsContract.getTreeDocumentId(treeUri);
            } catch (Throwable e) {
                treeDocId = android.provider.DocumentsContract.getDocumentId(treeUri);
            }
            if (treeDocId == null || treeDocId.isEmpty()) {
                return "ERROR: Could not resolve selected folder ID";
            }

            String folderName = null;
            try {
                android.net.Uri docUri = android.provider.DocumentsContract.buildDocumentUriUsingTree(treeUri, treeDocId);
                try (android.database.Cursor c = context.getContentResolver().query(
                        docUri,
                        new String[]{android.provider.DocumentsContract.Document.COLUMN_DISPLAY_NAME},
                        null, null, null)) {
                    if (c != null && c.moveToFirst() && !c.isNull(0)) {
                        String name = c.getString(0);
                        if (name != null && !name.trim().isEmpty()) {
                            folderName = name.trim();
                        }
                    }
                }
            } catch (Throwable ignored) {}

            if (folderName == null || folderName.isEmpty()) {
                String fallback = treeDocId;
                int slash = Math.max(fallback.lastIndexOf('/'), fallback.lastIndexOf(':'));
                if (slash >= 0 && slash + 1 < fallback.length()) {
                    fallback = fallback.substring(slash + 1).trim();
                }
                folderName = fallback.isEmpty() ? "imported_world" : fallback;
            }

            folderName = folderName.replaceAll("[/\\\\:*?\"<>|\\p{Cntrl}]", "_").trim();
            if (folderName.isEmpty() || ".".equals(folderName) || "..".equals(folderName)) {
                folderName = "imported_world";
            }

            java.io.File baseDir = new java.io.File(destDirPath);
            if (!baseDir.exists()) {
                baseDir.mkdirs();
            }
            java.io.File destFolder = new java.io.File(baseDir, folderName);
            int suffix = 1;
            while (destFolder.exists()) {
                destFolder = new java.io.File(baseDir, folderName + " (" + suffix + ")");
                suffix++;
            }
            if (!destFolder.mkdirs() && !destFolder.exists()) {
                return "ERROR: Failed to create destination folder " + destFolder.getName();
            }
            copyFolderChildren(context.getContentResolver(), treeUri, treeDocId, destFolder);
            return destFolder.getName();
        } catch (Throwable err) {
            android.util.Log.e("HorizonActivity", "copyFolderTree failed", err);
            String msg = err.getMessage();
            return "ERROR: " + (msg != null && !msg.isEmpty() ? msg : err.getClass().getSimpleName());
        }
    }

    private static void copyFolderChildren(android.content.ContentResolver resolver,
            android.net.Uri treeUri, String parentDocId, java.io.File destDir) {
        if (resolver == null || treeUri == null || parentDocId == null || destDir == null) return;
        if (!destDir.exists()) {
            destDir.mkdirs();
        }
        android.net.Uri childrenUri;
        try {
            childrenUri = android.provider.DocumentsContract.buildChildDocumentsUriUsingTree(treeUri, parentDocId);
        } catch (Throwable e) {
            android.util.Log.w("HorizonActivity", "Failed to build children URI for " + parentDocId, e);
            return;
        }
        String[] projection = new String[] {
            android.provider.DocumentsContract.Document.COLUMN_DOCUMENT_ID,
            android.provider.DocumentsContract.Document.COLUMN_DISPLAY_NAME,
            android.provider.DocumentsContract.Document.COLUMN_MIME_TYPE
        };
        java.util.ArrayList<String[]> entries = new java.util.ArrayList<>();
        try (android.database.Cursor c = resolver.query(childrenUri, projection, null, null, null)) {
            if (c != null) {
                while (c.moveToNext()) {
                    try {
                        String childDocId = c.isNull(0) ? null : c.getString(0);
                        String childName = c.isNull(1) ? null : c.getString(1);
                        String mimeType = c.isNull(2) ? null : c.getString(2);
                        if (childDocId != null && childName != null && !childName.isEmpty()) {
                            entries.add(new String[]{childDocId, childName, mimeType != null ? mimeType : ""});
                        }
                    } catch (Throwable rowErr) {
                        android.util.Log.w("HorizonActivity", "Skipped malformed cursor row", rowErr);
                    }
                }
            }
        } catch (Throwable queryErr) {
            android.util.Log.w("HorizonActivity", "Failed querying children for " + parentDocId, queryErr);
            return;
        }

        byte[] buffer = new byte[65536];
        for (String[] entry : entries) {
            String childDocId = entry[0];
            String rawName = entry[1];
            String mimeType = entry[2];
            String childName = rawName.replaceAll("[/\\\\:*?\"<>|\\p{Cntrl}]", "_").trim();
            if (childName.isEmpty() || ".".equals(childName) || "..".equals(childName)) {
                continue;
            }
            boolean isDir = android.provider.DocumentsContract.Document.MIME_TYPE_DIR.equals(mimeType);
            if (isDir) {
                try {
                    java.io.File subDir = new java.io.File(destDir, childName);
                    if (!subDir.exists()) subDir.mkdirs();
                    copyFolderChildren(resolver, treeUri, childDocId, subDir);
                } catch (Throwable dirErr) {
                    android.util.Log.w("HorizonActivity", "Failed copying subfolder " + childName, dirErr);
                }
            } else {
                java.io.File targetFile = new java.io.File(destDir, childName);
                try {
                    android.net.Uri childUri = android.provider.DocumentsContract.buildDocumentUriUsingTree(treeUri, childDocId);
                    try (java.io.InputStream in = resolver.openInputStream(childUri)) {
                        if (in != null) {
                            try (java.io.FileOutputStream out = new java.io.FileOutputStream(targetFile)) {
                                int read;
                                while ((read = in.read(buffer)) != -1) {
                                    out.write(buffer, 0, read);
                                }
                                out.flush();
                            }
                        } else if ("session.lock".equalsIgnoreCase(childName) && !targetFile.exists()) {
                            new java.io.FileOutputStream(targetFile).close();
                        }
                    }
                } catch (Throwable fileErr) {
                    android.util.Log.w("HorizonActivity", "Skipped unreadable file " + childName, fileErr);
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
