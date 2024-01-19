import 'package:http/http.dart' as http;
import 'dart:convert';

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
    // TODO(holzeis): this should come from the config
    const port = "3001";
    const host = "localhost";

    try {
      final response = await http.get(Uri.http('$host:$port', '/api/version'));

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
