# libwebp

`libwebp-1.6.0/` est la source officielle de Google (https://github.com/webmproject/libwebp,
tag `v1.6.0`), non modifiée. `build.bat` en compile `libwebp.dll` et
`libsharpyuv.dll` dans `../vendor-build/`, hors du dépôt, puis les copie à
côté de l'exécutable, qui les charge au démarrage (voir `src/webp.rs`).

## Compiler à la main

Nécessite CMake et MinGW-w64 **64 bits** (`mingw64`) : une DLL 32 bits ne
peut pas être chargée par un exécutable 64 bits. Avec MSYS2, placer
`mingw64\bin` en tête du PATH, sinon CMake peut choisir `mingw32`.

```
cmake -S vendor/libwebp-1.6.0 -B ../vendor-build -G Ninja ^
  -DBUILD_SHARED_LIBS=ON ^
  -DWEBP_BUILD_ANIM_UTILS=OFF -DWEBP_BUILD_CWEBP=OFF -DWEBP_BUILD_DWEBP=OFF ^
  -DWEBP_BUILD_GIF2WEBP=OFF -DWEBP_BUILD_IMG2WEBP=OFF -DWEBP_BUILD_VWEBP=OFF ^
  -DWEBP_BUILD_WEBPINFO=OFF -DWEBP_BUILD_WEBPMUX=OFF -DWEBP_BUILD_EXTRAS=OFF
cmake --build ../vendor-build --config Release
```

Les options `WEBP_BUILD_*=OFF` retirent les outils en ligne de commande et
les animations : seul l'encodeur d'images fixes sert.

## Mettre à jour libwebp

1. Remplacer `libwebp-X.Y.Z/` par la nouvelle source officielle et adapter
   `VENDOR_SRC` dans `build.bat`.
2. Supprimer `../vendor-build/` puis recompiler.
3. Vérifier avec `objdump -p libwebp.dll` :
   - que `WebPEncodeBGRA`, `WebPEncodeLosslessBGRA` et `WebPFree` sont
     toujours exportées ;
   - que la DLL ne dépend que de `libsharpyuv.dll` et de DLL système : ses
     dépendances ne sont cherchées que dans son dossier et dans System32.
