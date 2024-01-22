import 'dart:convert';
import 'package:get_10101/common/http_client.dart';

class Version {
  final String version;

  const Version({required this.version});

  factory Version.fromJson(Map<String, dynamic> json) {
    return switch (json) {
      {
        'version': String version,
      } =>
        Version(version: version),
      _ => throw const FormatException('Failed to load version.'),
    };
  }
}

class VersionService {
  const VersionService();

  Future<String> fetchVersion() async {
    try {
      final response = await HttpClientManager.instance.get(Uri(path: '/api/version'));

      if (response.statusCode == 200) {
        return Version.fromJson(jsonDecode(response.body) as Map<String, dynamic>).version;
      } else {
        return 'unknown';
      }
    } catch (e) {
      return "unknown";
    }
  }
}
