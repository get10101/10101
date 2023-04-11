import 'package:shared_preferences/shared_preferences.dart';

class Preferences {
  Preferences._privateConstructor();

  static final Preferences instance = Preferences._privateConstructor();

  static const userSeedBackupConfirmed = "userSeedBackupConfirmed";

  setUserSeedBackupConfirmed(bool value) async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    preferences.setBool(userSeedBackupConfirmed, value);
  }

  Future<bool> isUserSeedBackupConfirmed() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    return preferences.getBool(userSeedBackupConfirmed) ?? false;
  }
}
