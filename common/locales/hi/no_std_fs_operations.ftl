## क्षमता-आधारित I/O लागू करने के लिए std::fs ऑपरेशन प्रतिबंधित हैं।

no_std_fs_operations = std::fs ऑपरेशन `{ $operation }` क्षमता-आधारित फ़ाइल सिस्टम नीति को बायपास करता है।
    .note = std::fs परिवेश की कार्यशील निर्देशिका को छूता है; इसके बजाय `cap_std::fs::Dir` हैंडल और camino पथ स्वीकार करें, ताकि कॉलर क्षमता की सतह चुन सकें।
    .help = std::fs को सीधे कॉल करने के बजाय अपने API के माध्यम से `cap_std::fs::Dir` और `camino::Utf8Path`/`Utf8PathBuf` पैरामीटर पास करें।

no_std_fs_operations_unknown_keys = no_std_fs_operations { $version } (संशोधन { $revision }) के लिए [no_std_fs_operations] में अज्ञात कुंजियाँ { $keys } अनदेखी की जा रही हैं।
    .note = इस अनुभाग की अन्य कुंजियाँ अभी भी लागू हैं; अपरिचित कुंजियों का कोई प्रभाव नहीं है।
    .help = Whitaker सूट को ऐसे संशोधन में अद्यतन करें जो इन कुंजियों को समझता हो, या उन्हें dylint.toml से हटा दें।

no_std_fs_operations_discarded_section = no_std_fs_operations { $version } (संशोधन { $revision }) के लिए [no_std_fs_operations] कॉन्फ़िगरेशन छोड़ा जा रहा है: { $error }
    .note = कोई बहिष्करण लागू नहीं होता; lint अपनी डिफ़ॉल्ट सेटिंग्स का उपयोग कर रहा है।
    .help = dylint.toml में अमान्य मान ठीक करें, या Whitaker सूट को अद्यतन करें ताकि वह कुंजी को समझे।
