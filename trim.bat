@echo off
rem combine_all.bat – merge every *.rs, *.toml, *.py in this tree
setlocal enabledelayedexpansion

rem -------- output --------
set "OUT=combined.all.txt"

if exist "%OUT%" del "%OUT%"
if not exist "target" mkdir "target"

rem -------- merge files --------
for %%E in (rs toml py) do (
    for /f "delims=" %%F in ('
        dir /b /s /o:n *.%%E ^| findstr /v /i "\\target\\"
    ') do (
        echo ### --- %%~F --- >> "%OUT%"
        type "%%F" >> "%OUT%"
        echo.>> "%OUT%"
    )
)

echo Merged .rs, .toml, and .py files into %OUT%
endlocal
