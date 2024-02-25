import 'dart:convert';
import 'package:http/http.dart' as http;

class GitHubService {
  const GitHubService();

  Future<List<Meme>> fetchMemeImages() async {
    final response =
        await http.get(Uri.parse('https://api.github.com/repos/bonomat/memes/contents/images/'));

    if (response.statusCode == 200) {
      List<dynamic> data = jsonDecode(response.body);
      List<Meme> memes = data.map((item) => Meme.fromJson(item)).toList();
      return memes;
    } else {
      throw Exception('Failed to load meme images');
    }
  }
}

class Meme {
  final String downloadUrl;

  Meme({
    required this.downloadUrl,
  });

  factory Meme.fromJson(Map<String, dynamic> json) {
    return Meme(
      downloadUrl: json['download_url'],
    );
  }
}
