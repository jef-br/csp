# CSP test log — CiMini dataset run

Environment: Linux container, single-threaded (sequential, not the parallel `app::batch::run` path),
release build (`cargo build --release`), `src/bin/profile_run.rs` instrumented runner that calls
the same steps as `core::process_file` (`load → detect → geometry::plan → fill::render →
resize::resize_to_spec → save`) with an `Instant` timer around each step.

Dataset: 116 images assembled from `prism/test/datasets/CiMini` (flat files + 3 loose subfolders +
one zip archive extracted). Result: **116/116 ok, 0 failed**, total wall time **23.9s**.

## Summary

| Step | Total (ms) | Avg (ms) | % of total |
|---|---|---|---|
| load | 2062.3 | 17.78 | 8.6% |
| detect | 7882.8 | 67.96 | 33.0% |
| geometry (plan) | 0.0 | 0.00 | 0.0% |
| fill (render) | 6508.0 | 56.10 | 27.2% |
| resize | 4134.5 | 35.64 | 17.3% |
| save | 3303.9 | 28.48 | 13.8% |
| **total** | **23891.6** | **205.97** | 100% |

Median per-image total: 124.8 ms. P90: 317.4 ms.

**geometry::plan is free** — it's pure arithmetic on already-computed detection boxes, no image
buffers touched, so it never registers above 0.00 ms at this precision.

## Per-image detail

| File | W×H | load | detect | geometry | fill | resize | save | **total** | detail |
|---|---|---|---|---|---|---|---|---|---|
| 100267_1.jpg | 800×800 | 6.66 | 80.65 | 0.00 | 16.60 | 0.15 | 11.23 | **115.30** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.75 shadow=0.00 side=816 out=816 already_square=false |
| 100267_2.jpg | 800×800 | 7.36 | 75.99 | 0.00 | 7.71 | 0.29 | 13.26 | **104.61** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.35 shadow=0.00 side=845 out=845 already_square=false |
| 100267_3.jpg | 800×800 | 4.44 | 77.94 | 0.00 | 18.93 | 0.36 | 11.01 | **112.69** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.89 shadow=0.00 side=803 out=803 already_square=false |
| 100267_4.jpg | 800×800 | 4.05 | 72.62 | 0.00 | 19.36 | 0.33 | 11.55 | **107.92** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.70 shadow=0.00 side=828 out=828 already_square=false |
| 100267_5.jpg | 800×800 | 3.96 | 74.69 | 0.00 | 5.37 | 39.02 | 10.71 | **133.77** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.70 shadow=0.00 side=799 out=800 already_square=false |
| 100267_6  - BW001_c.jpg | 2500×2500 | 43.18 | 122.25 | 0.00 | 77.40 | 356.76 | 82.95 | **682.54** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.38 shadow=0.00 side=2125 out=2000 already_square=false |
| 100267_7 - BW001_c.jpg | 1000×1000 | 7.71 | 77.05 | 0.00 | 12.31 | 0.43 | 16.67 | **114.18** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.34 shadow=0.00 side=1003 out=1003 already_square=false |
| 133726012.jpg | 980×1470 | 12.19 | 57.16 | 0.00 | 68.53 | 0.97 | 36.96 | **175.82** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.20 shadow=0.00 side=1476 out=1476 already_square=false |
| 2021_3024_46_A - Copy.jpg | 1226×1226 | 21.44 | 75.29 | 0.00 | 12.75 | 0.49 | 27.70 | **137.68** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.10 shadow=0.00 side=1163 out=1163 already_square=false |
| 2021_3024_46_A.jpg | 2000×2000 | 18.77 | 95.10 | 0.00 | 228.14 | 344.61 | 68.66 | **755.30** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.64 shadow=0.00 side=2062 out=2000 already_square=false |
| 2021_3024_46_B.jpg | 2500×2500 | 36.81 | 119.29 | 0.00 | 381.69 | 448.02 | 69.47 | **1055.29** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.57 shadow=0.00 side=2537 out=2000 already_square=false |
| 2021_3041_65_A - Copy.jpg | 1466×1467 | 27.32 | 77.29 | 0.00 | 13.47 | 0.67 | 34.53 | **153.30** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.10 shadow=0.00 side=1312 out=1312 already_square=false |
| 2021_3041_65_A.jpg | 1237×1237 | 12.63 | 72.66 | 0.00 | 12.01 | 0.46 | 20.75 | **118.52** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.10 shadow=0.00 side=1084 out=1084 already_square=false |
| 23211008_02_A.jpg | 1538×2000 | 16.46 | 72.42 | 0.00 | 123.72 | 1.33 | 53.48 | **267.43** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.10 shadow=0.00 side=1782 out=1782 already_square=false |
| 23211008_02_B.jpg | 1230×1600 | 8.72 | 59.54 | 0.00 | 62.79 | 0.90 | 31.99 | **163.95** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.10 shadow=0.00 side=1387 out=1387 already_square=false |
| 23231096_35_A.jpg | 923×1200 | 7.56 | 58.07 | 0.00 | 31.24 | 0.47 | 23.64 | **121.00** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.43 shadow=0.00 side=1140 out=1140 already_square=false |
| 24211507_CARDIGAN_76_MAGENTA_B.jpg | 1384×1800 | 16.14 | 66.38 | 0.00 | 63.83 | 1.22 | 48.94 | **196.53** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.48 shadow=0.00 side=1625 out=1625 already_square=false |
| 24211511_86_A.jpg | 933×1200 | 8.55 | 60.04 | 0.00 | 23.09 | 0.51 | 21.76 | **113.96** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.68 shadow=0.00 side=1040 out=1040 already_square=false |
| 24211511_96_A.jpg | 923×1200 | 7.17 | 57.38 | 0.00 | 20.10 | 0.38 | 16.65 | **101.69** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.66 shadow=0.00 side=932 out=932 already_square=false |
| 2426834-7558_FW001_e.jpg | 2006×1337 | 40.08 | 63.43 | 0.00 | 24.79 | 0.78 | 41.60 | **170.69** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.43 shadow=0.00 side=1379 out=1379 already_square=false |
| 2426834-7558_bottom-packshot_sole.jpg | 709×1024 | 3.04 | 49.31 | 0.00 | 2.36 | 32.38 | 10.91 | **98.00** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.78 shadow=0.00 side=669 out=800 already_square=false |
| 2426834-7558_side-packshot_shoe.jpg | 709×1024 | 2.32 | 47.72 | 0.00 | 1.87 | 31.70 | 10.62 | **94.23** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.10 shadow=0.00 side=605 out=800 already_square=false |
| 2426834-7558_top-packshot_overhead.jpg | 709×1024 | 5.53 | 50.85 | 0.00 | 2.20 | 33.88 | 11.93 | **104.39** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.58 shadow=0.00 side=648 out=800 already_square=false |
| 26182-Denim-801_1.jpg | 980×1470 | 13.94 | 55.65 | 0.00 | 63.24 | 0.88 | 32.67 | **166.39** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.28 shadow=0.00 side=1353 out=1353 already_square=false |
| 26182-Denim-801_2.jpg | 980×1470 | 13.77 | 52.97 | 0.00 | 44.21 | 0.71 | 28.72 | **140.39** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.18 shadow=0.00 side=1264 out=1264 already_square=false |
| 26182-Denim-801_a (1).jpg | 980×1470 | 11.54 | 56.18 | 0.00 | 18.44 | 0.71 | 27.68 | **114.55** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.69 shadow=0.00 side=1261 out=1261 already_square=false |
| 4471-2290-B.jpg | 800×1200 | 8.45 | 51.06 | 0.00 | 7.40 | 0.23 | 15.09 | **82.24** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.84 shadow=0.00 side=905 out=905 already_square=false |
| 4471-2290-D.jpg | 800×1200 | 12.88 | 72.60 | 0.00 | 1.10 | 32.98 | 12.47 | **132.04** | kind=SalientSquare edges=0(t0 b0 l0 r0) conf=1.00 shadow=0.00 side=720 out=800 already_square=true |
| 4471-2290-F - Copy.jpg | 800×1200 | 8.33 | 50.82 | 0.00 | 6.25 | 0.25 | 14.26 | **79.91** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.86 shadow=0.00 side=880 out=880 already_square=false |
| 4471-2290-F.jpg | 800×1200 | 8.24 | 50.55 | 0.00 | 6.00 | 0.24 | 14.40 | **79.44** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.86 shadow=0.00 side=880 out=880 already_square=false |
| 5283-410-achterkant.jpg | 1028×1028 | 9.66 | 70.46 | 0.00 | 2.86 | 32.18 | 12.03 | **127.20** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.10 shadow=0.00 side=646 out=800 already_square=false |
| 5283-410.jpg | 1028×1028 | 9.29 | 71.66 | 0.00 | 3.18 | 32.67 | 11.52 | **128.33** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.10 shadow=0.00 side=686 out=800 already_square=false |
| 6318-5274.jpg | 750×1000 | 3.96 | 54.71 | 0.00 | 2.40 | 32.10 | 12.82 | **106.00** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.79 shadow=0.00 side=652 out=800 already_square=false |
| 8712345678901-2.jpg | 1067×1600 | 15.21 | 54.43 | 0.00 | 8.75 | 0.40 | 16.53 | **95.33** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.29 shadow=0.00 side=963 out=963 already_square=false |
| 8712345678901-3.jpg | 1067×1600 | 15.69 | 54.82 | 0.00 | 4.37 | 34.75 | 11.71 | **121.34** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.31 shadow=0.00 side=794 out=800 already_square=false |
| 8712345678901.jpg | 1067×1600 | 15.76 | 55.03 | 0.00 | 70.50 | 0.84 | 29.92 | **172.05** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.28 shadow=0.00 side=1302 out=1302 already_square=false |
| 87186790_1.jpg | 3543×5314 | 92.49 | 165.56 | 0.00 | 1654.08 | 749.24 | 65.19 | **2726.57** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.10 shadow=0.00 side=4394 out=2000 already_square=false |
| 87186790_2.jpg | 3543×5314 | 109.63 | 169.01 | 0.00 | 1036.28 | 573.99 | 66.56 | **1955.48** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.20 shadow=0.00 side=3567 out=2000 already_square=false |
| 90861071_b.jpg | 775×1024 | 5.68 | 61.87 | 0.00 | 7.97 | 0.37 | 20.08 | **95.97** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.69 shadow=0.00 side=992 out=992 already_square=false |
| 90861083_e.jpg | 800×1000 | 4.49 | 56.71 | 0.00 | 3.09 | 35.46 | 10.16 | **109.92** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.10 shadow=0.00 side=742 out=800 already_square=false |
| 99218629_det0.jpg | 800×800 | 2.18 | 69.70 | 0.00 | 2.79 | 37.21 | 11.14 | **123.03** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.39 shadow=0.00 side=664 out=800 already_square=false |
| 99218629_det1.jpg | 800×800 | 2.40 | 71.42 | 0.00 | 7.00 | 32.64 | 11.74 | **125.20** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.66 shadow=0.00 side=564 out=800 already_square=false |
| 99218629_det2.jpg | 800×800 | 1.88 | 67.88 | 0.00 | 9.43 | 31.41 | 10.37 | **120.98** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.50 shadow=0.00 side=564 out=800 already_square=false |
| 99218793_det1.jpg | 800×800 | 4.15 | 267.63 | 0.00 | 2.52 | 32.16 | 10.90 | **317.38** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.10 shadow=0.00 side=646 out=800 already_square=false |
| 99218809_det0.jpg | 800×800 | 2.16 | 304.46 | 0.00 | 3.53 | 33.43 | 10.73 | **354.31** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.54 shadow=0.00 side=728 out=800 already_square=false |
| 99218809_det1.jpg | 800×800 | 2.18 | 275.86 | 0.00 | 3.03 | 33.21 | 11.09 | **325.39** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.49 shadow=0.00 side=720 out=800 already_square=false |
| 99218810_det1.jpg | 800×800 | 1.84 | 268.75 | 0.00 | 2.82 | 32.30 | 10.50 | **316.22** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.32 shadow=0.00 side=669 out=800 already_square=false |
| 99984901_99984901_det0.jpg | 980×1470 | 13.28 | 57.40 | 0.00 | 12.06 | 0.54 | 21.96 | **105.25** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.73 shadow=0.00 side=1049 out=1049 already_square=false |
| 99984901_99984901_det1.jpg | 980×1470 | 14.81 | 59.27 | 0.00 | 16.91 | 0.61 | 28.74 | **120.35** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.78 shadow=0.00 side=1238 out=1238 already_square=false |
| AY_FFK0230_83035_01 FW001_b.png | 1500×2000 | 48.23 | 3.61 | 0.00 | 34.29 | 1.06 | 39.84 | **127.04** | kind=Subject edges=0(t0 b0 l0 r0) conf=1.00 shadow=0.00 side=1565 out=1565 already_square=false |
| AY_FFK0230_83035_02FW001_A.png | 1500×2000 | 52.82 | 4.95 | 0.00 | 30.03 | 0.93 | 40.24 | **128.98** | kind=Subject edges=0(t0 b0 l0 r0) conf=1.00 shadow=0.00 side=1468 out=1468 already_square=false |
| AY_FFK0230_83035_03FW001_c.png | 1500×2000 | 73.70 | 3.82 | 0.00 | 4.39 | 0.94 | 42.03 | **124.90** | kind=WholeFrame edges=4(t1 b1 l1 r1) conf=0.20 shadow=0.00 side=1500 out=1500 already_square=true |
| AY_FFK0230_83035_04.png | 1500×1500 | 43.99 | 2.63 | 0.00 | 75.97 | 0.95 | 41.93 | **165.48** | kind=Subject edges=0(t0 b0 l0 r0) conf=1.00 shadow=0.00 side=1592 out=1592 already_square=false |
| AY_FFK0230_83035_05FW001_d.png | 1500×2000 | 47.71 | 3.59 | 0.00 | 25.88 | 0.91 | 33.99 | **112.09** | kind=Subject edges=0(t0 b0 l0 r0) conf=1.00 shadow=0.00 side=1441 out=1441 already_square=false |
| AY_FFK0230_83035_06FW001_f.png | 1125×1500 | 35.39 | 2.05 | 0.00 | 14.47 | 0.46 | 25.34 | **77.72** | kind=Subject edges=2(t0 b0 l1 r1) conf=1.00 shadow=0.00 side=1125 out=1125 already_square=false |
| C153KB460011_Cedric_City_Grey_BAC.png | 880×1320 | 25.56 | 1.41 | 0.00 | 25.88 | 0.85 | 35.94 | **89.65** | kind=Subject edges=0(t0 b0 l0 r0) conf=1.00 shadow=0.00 side=1410 out=1410 already_square=false |
| C153KB460011_Cedric_City_Grey_FRON.png | 862×1300 | 25.09 | 1.27 | 0.00 | 23.62 | 0.81 | 33.57 | **84.36** | kind=WholeFrame edges=1(t1 b0 l0 r0) conf=0.20 shadow=0.00 side=1355 out=1355 already_square=false |
| C153KB460011_Cedric__City_Grey_DETAIL.png | 800×1500 | 25.20 | 1.59 | 0.00 | 58.71 | 0.91 | 35.59 | **122.00** | kind=Subject edges=0(t0 b0 l0 r0) conf=1.00 shadow=0.00 side=1410 out=1410 already_square=false |
| C153KU420009_Kendall_Twill sand_BACK.png | 700×1500 | 22.76 | 1.20 | 0.00 | 57.09 | 0.80 | 34.94 | **116.81** | kind=Subject edges=0(t0 b0 l0 r0) conf=1.00 shadow=0.00 side=1410 out=1410 already_square=false |
| C153KU420009_Kendall_Twill sand_DETAIL1.png | 900×1500 | 21.17 | 1.57 | 0.00 | 71.72 | 0.87 | 33.43 | **128.76** | kind=Subject edges=0(t0 b0 l0 r0) conf=1.00 shadow=0.00 side=1410 out=1410 already_square=false |
| C153KU420009_Kendall_Twill sand_FRONT.png | 1000×1500 | 24.38 | 1.81 | 0.00 | 66.24 | 0.72 | 35.36 | **128.53** | kind=Subject edges=0(t0 b0 l0 r0) conf=1.00 shadow=0.00 side=1410 out=1410 already_square=false |
| CARDIGAN_MAGENTA76_A.jpg | 1538×2000 | 20.15 | 71.30 | 0.00 | 113.23 | 1.58 | 64.42 | **270.68** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.54 shadow=0.00 side=1873 out=1873 already_square=false |
| CARDIGAN_MAGENTA76_DETAIL.jpg | 959×1013 | 6.29 | 68.61 | 0.00 | 7.97 | 31.58 | 12.18 | **126.63** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.19 shadow=0.00 side=564 out=800 already_square=false |
| IMG_2619_indigo.png | 688×1024 | 3.05 | 47.54 | 0.00 | 21.25 | 0.28 | 12.20 | **84.32** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.92 shadow=0.00 side=826 out=826 already_square=false |
| IMG_4432.png | 1000×1500 | 24.90 | 1.78 | 0.00 | 80.15 | 1.08 | 44.84 | **152.75** | kind=Subject edges=0(t0 b0 l0 r0) conf=1.00 shadow=0.00 side=1604 out=1604 already_square=false |
| IMG_7710.jpg | 768×1024 | 4.14 | 58.99 | 0.00 | 24.80 | 0.42 | 17.86 | **106.22** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.59 shadow=0.00 side=1002 out=1002 already_square=false |
| IMG_9021.jpg | 683×1024 | 3.12 | 50.10 | 0.00 | 17.82 | 0.30 | 13.52 | **84.87** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.77 shadow=0.00 side=885 out=885 already_square=false |
| OMB-E129-TGV_1.jpg | 1538×1980 | 23.01 | 68.69 | 0.00 | 12.84 | 0.51 | 23.52 | **128.58** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.71 shadow=0.00 side=1130 out=1130 already_square=false |
| OMB-E129-TGV_2.jpg | 1538×1980 | 24.37 | 69.54 | 0.00 | 14.67 | 0.51 | 25.10 | **134.20** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.72 shadow=0.00 side=1165 out=1165 already_square=false |
| OMB-E129-TGV_3.jpg | 1538×1980 | 25.06 | 68.22 | 0.00 | 16.39 | 0.53 | 25.38 | **135.58** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.67 shadow=0.00 side=1188 out=1188 already_square=false |
| OMB-E129-TGV_4.jpg | 1491×1920 | 16.24 | 67.76 | 0.00 | 16.66 | 0.63 | 27.63 | **128.92** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.24 shadow=0.00 side=1204 out=1204 already_square=false |
| OMB-E166-BV_1-back.jpg | 1924×2474 | 24.33 | 81.95 | 0.00 | 41.61 | 1.66 | 63.84 | **213.40** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.44 shadow=0.00 side=1908 out=1908 already_square=false |
| OMB-E166-BV_1-front.jpg | 1924×2474 | 32.87 | 86.70 | 0.00 | 50.26 | 1.67 | 65.60 | **237.11** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.44 shadow=0.00 side=1908 out=1908 already_square=false |
| OMB-E166-BV_1.jpg | 1924×2474 | 28.44 | 87.16 | 0.00 | 48.72 | 1.69 | 60.68 | **226.70** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.34 shadow=0.00 side=1908 out=1908 already_square=false |
| OMB-E166-BV_2.jpg | 1924×2474 | 32.76 | 87.42 | 0.00 | 49.98 | 1.64 | 65.28 | **237.08** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.44 shadow=0.00 side=1908 out=1908 already_square=false |
| OMB-E166-BV_3.jpg | 1924×2474 | 31.72 | 87.01 | 0.00 | 175.76 | 1.75 | 108.22 | **404.47** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.52 shadow=0.00 side=1926 out=1926 already_square=false |
| OMB-E166-BV_4.jpg | 1924×2474 | 37.39 | 90.64 | 0.00 | 73.39 | 348.05 | 71.03 | **620.51** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.34 shadow=0.00 side=2132 out=2000 already_square=false |
| OMB-E180-BV_1.jpg | 1538×1980 | 29.30 | 72.11 | 0.00 | 9.69 | 0.37 | 20.20 | **131.69** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.73 shadow=0.00 side=1023 out=1023 already_square=false |
| OMB-E180-BV_2.jpg | 1538×1980 | 23.53 | 69.19 | 0.00 | 10.36 | 0.40 | 21.17 | **124.66** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.61 shadow=0.00 side=1054 out=1054 already_square=false |
| OMB-E180-BV_3.jpg | 1538×1980 | 23.17 | 68.51 | 0.00 | 9.12 | 0.36 | 19.95 | **121.12** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.73 shadow=0.00 side=1023 out=1023 already_square=false |
| OMB-E180-BV_4.jpg | 1538×1980 | 21.51 | 68.63 | 0.00 | 7.67 | 0.35 | 16.97 | **115.14** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.74 shadow=0.00 side=924 out=924 already_square=false |
| OMB-E180-BV_5.jpg | 1538×1980 | 24.15 | 70.98 | 0.00 | 18.38 | 0.70 | 29.04 | **143.26** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.54 shadow=0.00 side=1241 out=1241 already_square=false |
| OMB-E180-BV_6.jpg | 1624×2080 | 26.30 | 86.16 | 0.00 | 71.91 | 352.34 | 79.11 | **615.83** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.53 shadow=0.00 side=2131 out=2000 already_square=false |
| OMB-E181-CVW_1.jpg | 1538×1980 | 27.89 | 72.55 | 0.00 | 8.61 | 0.42 | 17.54 | **127.02** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.88 shadow=0.00 side=951 out=951 already_square=false |
| OMB-E181-CVW_2.jpg | 1538×1980 | 22.32 | 69.73 | 0.00 | 8.89 | 0.37 | 19.78 | **121.11** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.76 shadow=0.00 side=1017 out=1017 already_square=false |
| OMB-E181-CVW_3.jpg | 1538×1980 | 23.90 | 69.62 | 0.00 | 9.10 | 0.40 | 17.92 | **120.95** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.89 shadow=0.00 side=959 out=959 already_square=false |
| OMB-E181-CVW_4.jpg | 1538×1980 | 23.32 | 70.65 | 0.00 | 8.79 | 0.46 | 18.04 | **121.26** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.92 shadow=0.00 side=963 out=963 already_square=false |
| OMB-E181-CVW_5.jpg | 1538×1980 | 23.43 | 72.17 | 0.00 | 9.67 | 0.39 | 20.86 | **126.52** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.53 shadow=0.00 side=1023 out=1023 already_square=false |
| OMB-E181-CVW_6.jpg | 1150×1480 | 9.21 | 64.06 | 0.00 | 6.18 | 0.23 | 15.25 | **94.93** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.55 shadow=0.00 side=869 out=869 already_square=false |
| Pareo Exotica.jpg | 1000×1500 | 9.16 | 52.77 | 0.00 | 4.12 | 35.06 | 12.51 | **113.63** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.40 shadow=0.00 side=784 out=800 already_square=false |
| Pareo_exotica_F1.jpg | 1000×1500 | 15.73 | 54.75 | 0.00 | 87.05 | 1.10 | 46.24 | **204.87** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.34 shadow=0.00 side=1578 out=1578 already_square=false |
| Pareo_exotica_F2.jpg | 667×1000 | 7.61 | 48.77 | 0.00 | 29.91 | 0.53 | 20.18 | **107.01** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.22 shadow=0.00 side=1062 out=1062 already_square=false |
| TH12_0N3_M47CH32_70_R3D_36R37.jpg | 1385×2000 | 11.51 | 65.39 | 0.00 | 27.55 | 0.89 | 34.40 | **139.76** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.65 shadow=0.00 side=1421 out=1421 already_square=false |
| TH12_0N3_M47CH32_70_R3D_36R37_AS_4_FR0N7.jpg | 1385×2000 | 11.70 | 66.86 | 0.00 | 27.67 | 0.89 | 35.20 | **142.32** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.63 shadow=0.00 side=1421 out=1421 already_square=false |
| T_SHIRT_EGRET_DETAIL.jpg | 1385×2000 | 21.62 | 64.90 | 0.00 | 18.50 | 0.74 | 32.76 | **138.53** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.10 shadow=0.00 side=1294 out=1294 already_square=false |
| a (1).jpg | 980×1470 | 11.62 | 55.33 | 0.00 | 18.59 | 0.66 | 28.05 | **114.26** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.69 shadow=0.00 side=1261 out=1261 already_square=false |
| blue-hoodie.jpg | 600×801 | 2.05 | 56.36 | 0.00 | 2.83 | 32.84 | 11.03 | **105.12** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.74 shadow=0.00 side=643 out=800 already_square=false |
| blue.jpg | 683×1024 | 3.00 | 48.78 | 0.00 | 2.60 | 33.39 | 11.51 | **99.29** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.65 shadow=0.00 side=675 out=800 already_square=false |
| charcol-wrap.jpg | 745×1024 | 3.71 | 54.75 | 0.00 | 25.78 | 0.39 | 16.87 | **101.51** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.80 shadow=0.00 side=983 out=983 already_square=false |
| foldercontainsID99984905_1.jpg | 1040×1560 | 7.99 | 53.61 | 0.00 | 69.30 | 0.73 | 28.74 | **160.38** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.53 shadow=0.00 side=1310 out=1310 already_square=false |
| foldercontainsID99984905_2.jpg | 1467×2200 | 24.35 | 64.82 | 0.00 | 170.61 | 1.55 | 59.18 | **320.52** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.55 shadow=0.00 side=1851 out=1851 already_square=false |
| foldercontainsID99984905_3.jpg | 980×1470 | 12.89 | 56.38 | 0.00 | 12.01 | 0.48 | 19.93 | **101.70** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.14 shadow=0.00 side=999 out=999 already_square=false |
| foldercontainsID99984905_4.jpg | 980×1470 | 13.15 | 53.64 | 0.00 | 12.01 | 0.48 | 21.53 | **100.82** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.15 shadow=0.00 side=1055 out=1055 already_square=false |
| graphite-scarf.jpg | 823×823 | 7.52 | 80.92 | 0.00 | 4.06 | 34.35 | 12.55 | **139.42** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.81 shadow=0.00 side=722 out=800 already_square=false |
| green-sweater-back.jpg | 683×1024 | 4.28 | 52.52 | 0.00 | 5.61 | 35.27 | 12.42 | **110.11** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.86 shadow=0.00 side=798 out=800 already_square=false |
| green-sweater-front.jpg | 683×1024 | 4.19 | 52.83 | 0.00 | 4.98 | 35.20 | 13.82 | **111.03** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.86 shadow=0.00 side=798 out=800 already_square=false |
| grey-scarf.jpg | 600×1024 | 2.68 | 42.55 | 0.00 | 3.10 | 33.05 | 11.25 | **92.64** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.71 shadow=0.00 side=660 out=800 already_square=false |
| sweater-detail.jpg | 941×940 | 6.11 | 99.76 | 0.00 | 1.53 | 0.22 | 14.37 | **122.00** | kind=SalientSquare edges=0(t0 b0 l0 r0) conf=1.00 shadow=0.00 side=846 out=846 already_square=true |
| triggered-mistery.jpg | 980×1470 | 8.31 | 50.88 | 0.00 | 10.83 | 31.00 | 11.38 | **112.41** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.24 shadow=0.00 side=564 out=800 already_square=false |
| triggered-tshirt-main.jpg | 980×1470 | 9.52 | 53.76 | 0.00 | 10.99 | 0.50 | 18.98 | **93.76** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.65 shadow=0.00 side=1032 out=1032 already_square=false |
| triggered_black-tshirt-back-americain.jpg | 980×1470 | 15.04 | 61.13 | 0.00 | 45.45 | 0.79 | 34.17 | **156.60** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.47 shadow=0.00 side=1376 out=1376 already_square=false |
| triggered_black-tshirt-front-americain.jpg | 980×1470 | 15.40 | 57.09 | 0.00 | 64.57 | 1.02 | 41.36 | **179.45** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.42 shadow=0.00 side=1520 out=1520 already_square=false |
| triggered_black-tshirt-front-detail.jpg | 677×677 | 5.81 | 68.76 | 0.00 | 2.86 | 32.32 | 12.35 | **122.12** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.14 shadow=0.00 side=667 out=800 already_square=false |
| triggered_black-tshirt-front-silhouette.jpg | 1533×2300 | 22.61 | 69.35 | 0.00 | 158.21 | 1.63 | 60.86 | **312.66** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.39 shadow=0.00 side=1873 out=1873 already_square=false |
| triggered_ghost-front.jpg | 980×1470 | 8.62 | 55.68 | 0.00 | 12.43 | 0.47 | 18.70 | **95.92** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.66 shadow=0.00 side=1044 out=1044 already_square=false |
| triggered_ghost_back.jpg | 980×1470 | 8.31 | 53.59 | 0.00 | 12.25 | 0.47 | 18.29 | **92.91** | kind=Subject edges=0(t0 b0 l0 r0) conf=0.67 shadow=0.00 side=1038 out=1038 already_square=false |

## Column notes

- **detail** column decodes as: `kind` = `DetectionKind` (`Subject` / `SalientSquare` / `WholeFrame`);
  `edges=N(t b l r)` = how many of the 4 image edges the detected box was judged to intersect, and which;
  `conf` = `Detection.confidence` (0–1); `shadow` = `Detection.hard_shadow_fraction` (0–1, see findings doc —
  always 0.00 in this run); `side` = planned fill-target square side in original-image pixels before the
  final clamp/resize; `out` = actual output square side in pixels (clamped to `[800, 2000]`);
  `already_square` = whether `fill::render` was skipped entirely.
