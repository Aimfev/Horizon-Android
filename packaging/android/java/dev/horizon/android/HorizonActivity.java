package dev.horizon.android;

/** SAF result callback only. The native activity, UI and file operations stay Rust. */
public final class HorizonActivity extends android.app.NativeActivity {
    private static final int REQUEST_MOD_PERMISSIONS = 3;
    private static final String RELEASE_SIGNER =
            "b83e95a3ff2ff82509300f132506d61b1ca3572a61e80b8588f8be815f7b9b05";
    private boolean loaded;
    private java.io.File nativeLibrary;

    private static volatile boolean sCancelFolderCopy = false;
    private static volatile int sCopyFilesDone = 0;
    private static volatile int sCopyFilesTotal = 0;
    private static volatile String sCopyCurrentFile = "";

    private native void nativeDocumentResult(int request, int result, String uri);

    public static void cancelFolderCopy() {
        sCancelFolderCopy = true;
    }

    public static String getFolderCopyProgress() {
        return sCopyFilesDone + "|" + sCopyFilesTotal + "|" + (sCopyCurrentFile != null ? sCopyCurrentFile : "");
    }

    @Override protected void onCreate(android.os.Bundle savedInstanceState) {
        loadGame();
        super.onCreate(savedInstanceState);
        applyHighRefreshRate(this, 0);
    }

    private void loadGame() {
        if (loaded) {
            return;
        }
        loaded = true;
        java.io.File build = NativeMods.chosen(this, versionName());
        if (build != null) {
            try {
                android.system.Os.setenv("HORIZON_GAME_LIBRARY", build.getPath(), true);
                System.load(build.getPath());
                nativeLibrary = build;
                return;
            } catch (Throwable e) {
                NativeMods.setAside(
                        this,
                        "The native-mod build could not be loaded ("
                                + e.getClass().getSimpleName()
                                + "); starting without native mods.");
            }
        }
        try {
            android.system.Os.unsetenv("HORIZON_GAME_LIBRARY");
        } catch (android.system.ErrnoException ignored) {
        }
        System.loadLibrary("horizon_android");
    }

    public String versionName() {
        try {
            String v = getPackageManager().getPackageInfo(getPackageName(), 0).versionName;
            return v != null ? v : "";
        } catch (android.content.pm.PackageManager.NameNotFoundException e) {
            return "";
        }
    }

    @SuppressWarnings("deprecation")
    public boolean releaseBuild() {
        try {
            android.content.pm.Signature[] sigs =
                    getPackageManager()
                            .getPackageInfo(
                                    getPackageName(),
                                    android.content.pm.PackageManager.GET_SIGNATURES)
                            .signatures;
            if (sigs == null || sigs.length == 0) {
                return false;
            }
            byte[] digest =
                    java.security.MessageDigest.getInstance("SHA-256")
                            .digest(sigs[0].toByteArray());
            StringBuilder hex = new StringBuilder(digest.length * 2);
            for (byte b : digest) {
                hex.append(String.format("%02x", b & 0xff));
            }
            return RELEASE_SIGNER.equals(hex.toString());
        } catch (Exception e) {
            return false;
        }
    }

    public void firstFrame() {
        NativeMods.started(this);
        askModPermissions();
    }

    private void askModPermissions() {
        if (nativeLibrary == null) {
            return;
        }
        final java.util.Map<String, java.util.List<String>> wanted = NativeMods.permissions(nativeLibrary);
        if (wanted == null || wanted.isEmpty() || NativeMods.permissionsDeclined(nativeLibrary)) {
            return;
        }
        java.util.ArrayList<String> permList = new java.util.ArrayList<>();
        for (String key : wanted.keySet()) {
            if ("microphone".equalsIgnoreCase(key)) {
                permList.add(android.Manifest.permission.RECORD_AUDIO);
            }
        }
        if (permList.isEmpty()) {
            return;
        }
        final String[] permissions = permList.toArray(new String[0]);
        runOnUiThread(
                new Runnable() {
                    @Override
                    public void run() {
                        java.util.ArrayList<String> missing = new java.util.ArrayList<>();
                        for (String p : permissions) {
                            if (checkSelfPermission(p)
                                    != android.content.pm.PackageManager.PERMISSION_GRANTED) {
                                missing.add(p);
                            }
                        }
                        if (!missing.isEmpty()) {
                            requestPermissions(
                                    missing.toArray(new String[0]), REQUEST_MOD_PERMISSIONS);
                        }
                    }
                });
    }

    public void restart() {
        runOnUiThread(
                new Runnable() {
                    @Override
                    public void run() {
                        android.content.Intent launch =
                                getPackageManager().getLaunchIntentForPackage(getPackageName());
                        if (launch == null) {
                            launch =
                                    new android.content.Intent(
                                            HorizonActivity.this, HorizonActivity.class);
                        }
                        launch.addFlags(
                                android.content.Intent.FLAG_ACTIVITY_NEW_TASK
                                        | android.content.Intent.FLAG_ACTIVITY_CLEAR_TASK);
                        startActivity(launch);
                        Runtime.getRuntime().exit(0);
                    }
                });
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
                    float targetRate = 0.0f;
                    if (best != null) {
                        lp.preferredDisplayModeId = best.getModeId();
                        lp.preferredRefreshRate = best.getRefreshRate();
                        targetRate = best.getRefreshRate();
                    } else if (preferredHz > 0) {
                        lp.preferredRefreshRate = (float) preferredHz;
                        targetRate = (float) preferredHz;
                    }
                    window.setAttributes(lp);

                    if (android.os.Build.VERSION.SDK_INT >= 30) {
                        try {
                            android.view.View decor = window.getDecorView();
                            if (decor instanceof android.view.ViewGroup) {
                                android.view.ViewGroup vg = (android.view.ViewGroup) decor;
                                for (int i = 0; i < vg.getChildCount(); i++) {
                                    android.view.View child = vg.getChildAt(i);
                                    if (child instanceof android.view.SurfaceView) {
                                        android.view.Surface s = ((android.view.SurfaceView) child).getHolder().getSurface();
                                        if (s != null && s.isValid()) {
                                            s.setFrameRate(targetRate, android.view.Surface.FRAME_RATE_COMPATIBILITY_DEFAULT);
                                        }
                                    }
                                }
                            }
                        } catch (Throwable ignored) {}
                    }
                } catch (Throwable ignored) {}
            }
        });
    }

    @Override protected void onActivityResult(int request, int result, android.content.Intent data) {
        super.onActivityResult(request, result, data);
        if (request != 4101 && request != 4102 && request != 4103) return;
        try {
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
        } catch (Throwable err) {
            android.util.Log.e("HorizonActivity", "onActivityResult failed", err);
            try {
                nativeDocumentResult(request, 0, null);
            } catch (Throwable ignored) {}
        }
    }

    public static String copyFolderTree(android.content.Context context, String treeUriString, String destDirPath) {
        sCancelFolderCopy = false;
        sCopyFilesDone = 0;
        sCopyFilesTotal = 0;
        sCopyCurrentFile = "Scanning folder...";
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

            int[] totalCounter = new int[]{0};
            countFolderFiles(context.getContentResolver(), treeUri, treeDocId, 0, totalCounter);
            sCopyFilesTotal = Math.max(totalCounter[0], 1);

            int[] fileCount = new int[]{0};
            copyFolderChildren(context.getContentResolver(), treeUri, treeDocId, destFolder, 0, fileCount);
            if (sCancelFolderCopy) {
                deleteRecursive(destFolder);
                return "ERROR: Import cancelled";
            }
            if (fileCount[0] == 0) {
                deleteRecursive(destFolder);
                return "ERROR: Selected folder was empty or required direct SAF stream";
            }
            return destFolder.getName();
        } catch (Throwable err) {
            android.util.Log.e("HorizonActivity", "copyFolderTree failed", err);
            String msg = err.getMessage();
            return "ERROR: " + (msg != null && !msg.isEmpty() ? msg : err.getClass().getSimpleName());
        }
    }

    private static void deleteRecursive(java.io.File f) {
        if (f == null || !f.exists()) return;
        if (f.isDirectory()) {
            java.io.File[] children = f.listFiles();
            if (children != null) {
                for (java.io.File c : children) {
                    deleteRecursive(c);
                }
            }
        }
        try { f.delete(); } catch (Throwable ignored) {}
    }

    private static void countFolderFiles(android.content.ContentResolver resolver,
            android.net.Uri treeUri, String parentDocId, int depth, int[] count) {
        if (sCancelFolderCopy || resolver == null || treeUri == null || parentDocId == null) return;
        if (depth > 20 || count[0] > 20000) return;
        android.net.Uri childrenUri;
        try {
            childrenUri = android.provider.DocumentsContract.buildChildDocumentsUriUsingTree(treeUri, parentDocId);
        } catch (Throwable e) {
            return;
        }
        String[] projection = new String[] {
            android.provider.DocumentsContract.Document.COLUMN_DOCUMENT_ID,
            android.provider.DocumentsContract.Document.COLUMN_MIME_TYPE
        };
        java.util.ArrayList<String> subDirs = new java.util.ArrayList<>();
        try (android.database.Cursor c = resolver.query(childrenUri, projection, null, null, null)) {
            if (c != null) {
                while (!sCancelFolderCopy && c.moveToNext()) {
                    String childId = c.isNull(0) ? null : c.getString(0);
                    String mime = c.isNull(1) ? "" : c.getString(1);
                    if (childId == null) continue;
                    if (android.provider.DocumentsContract.Document.MIME_TYPE_DIR.equals(mime)) {
                        subDirs.add(childId);
                    } else {
                        count[0]++;
                    }
                }
            }
        } catch (Throwable ignored) {}
        for (String subId : subDirs) {
            if (sCancelFolderCopy) return;
            countFolderFiles(resolver, treeUri, subId, depth + 1, count);
        }
    }

    private static void copyFolderChildren(android.content.ContentResolver resolver,
            android.net.Uri treeUri, String parentDocId, java.io.File destDir) {
        copyFolderChildren(resolver, treeUri, parentDocId, destDir, 0, new int[]{0});
    }

    private static void copyFolderChildren(android.content.ContentResolver resolver,
            android.net.Uri treeUri, String parentDocId, java.io.File destDir, int depth, int[] fileCount) {
        if (sCancelFolderCopy || resolver == null || treeUri == null || parentDocId == null || destDir == null) return;
        if (depth > 20 || fileCount[0] > 20000) return;
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
                while (!sCancelFolderCopy && c.moveToNext()) {
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
            if (sCancelFolderCopy) return;
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
                    copyFolderChildren(resolver, treeUri, childDocId, subDir, depth + 1, fileCount);
                } catch (Throwable dirErr) {
                    android.util.Log.w("HorizonActivity", "Failed copying subfolder " + childName, dirErr);
                }
            } else {
                if (++fileCount[0] > 20000) break;
                sCopyCurrentFile = childName;
                java.io.File targetFile = new java.io.File(destDir, childName);
                try {
                    android.net.Uri childUri = android.provider.DocumentsContract.buildDocumentUriUsingTree(treeUri, childDocId);
                    try (java.io.InputStream in = resolver.openInputStream(childUri)) {
                        if (in != null) {
                            try (java.io.FileOutputStream out = new java.io.FileOutputStream(targetFile)) {
                                int read;
                                while (!sCancelFolderCopy && (read = in.read(buffer)) != -1) {
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
                sCopyFilesDone = fileCount[0];
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
